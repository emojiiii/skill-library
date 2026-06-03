use flate2::write::GzEncoder;
use mockito::Matcher;
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ArchiveProvider, FileKind, GitRef, PermissionLevel, ProviderError, SkillSourceProvider,
    SourceRef,
};
use std::io::Write;

use crate::util::redact_access_token;
use crate::GiteeProvider;

#[test]
fn gitee_provider_reports_instance_id() {
    let provider =
        GiteeProvider::with_instance_base_url("gitee.enterprise", "https://gitee/api/v5", None)
            .unwrap();

    assert_eq!(SkillSourceProvider::id(&provider), "gitee.enterprise");
}

#[test]
fn redact_access_token_preserves_query_shape() {
    let redacted =
        redact_access_token("/api/v5/repos/acme/team?access_token=secret-token&recursive=1");

    assert_eq!(
        redacted,
        "/api/v5/repos/acme/team?access_token=[REDACTED]&recursive=1"
    );
    assert!(!redacted.contains("secret-token"));
}

#[tokio::test]
async fn get_source_maps_repo_metadata() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v5/repos/acme/team-skills")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(repo_json(
            "acme/team-skills",
            true,
            r#""permissions":{"admin":false,"push":true,"pull":true}"#,
        ))
        .create_async()
        .await;
    let provider = GiteeProvider::anonymous(format!("{}/api/v5", server.url())).unwrap();
    let workspace = workspace("acme", "team-skills");

    let source = provider.get_source(&workspace).await.unwrap();

    mock.assert_async().await;
    assert_eq!(source.provider, "gitee.com");
    assert_eq!(source.owner, "acme");
    assert_eq!(source.repo, "team-skills");
    assert_eq!(source.permission, PermissionLevel::Write);
}

#[tokio::test]
async fn list_files_maps_git_tree_entries() {
    let mut server = mockito::Server::new_async().await;
    let repo = server
        .mock("GET", "/api/v5/repos/acme/team-skills")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(repo_json(
            "acme/team-skills",
            false,
            r#""permissions":{"admin":false,"push":false,"pull":true}"#,
        ))
        .create_async()
        .await;
    let tree = server
        .mock("GET", "/api/v5/repos/acme/team-skills/git/trees/master")
        .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "tree": [
                    {"sha":"a","path":"skills/code/SKILL.md","type":"blob","size":9},
                    {"sha":"b","path":"skills","type":"tree"},
                    {"sha":"c","path":"vendor/lib","type":"commit"}
                ]
            }"#,
        )
        .create_async()
        .await;
    let provider = GiteeProvider::anonymous(format!("{}/api/v5", server.url())).unwrap();
    let workspace = workspace("acme", "team-skills");

    let files = provider
        .list_files(&workspace, &SourceRef::Latest)
        .await
        .unwrap();

    repo.assert_async().await;
    tree.assert_async().await;
    assert_eq!(files.len(), 3);
    assert!(matches!(files[0].kind, FileKind::File));
    assert!(matches!(files[1].kind, FileKind::Directory));
    assert!(matches!(files[2].kind, FileKind::Submodule));
    assert_eq!(files[0].size, Some(9));
}

#[tokio::test]
async fn read_file_reads_raw_bytes() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock(
            "GET",
            "/api/v5/repos/acme/team-skills/raw/skills%2Fcode%2FSKILL.md",
        )
        .match_query(Matcher::UrlEncoded("ref".into(), "master".into()))
        .with_status(200)
        .with_header("x-gitee-blob-id", "blob-sha")
        .with_body(Vec::from(&b"# hi\n"[..]))
        .create_async()
        .await;
    let provider = GiteeProvider::anonymous(format!("{}/api/v5", server.url())).unwrap();
    let workspace = workspace("acme", "team-skills");

    let blob = provider
        .read_file(
            &workspace,
            &SourceRef::Git(GitRef::Branch("master".to_owned())),
            "skills/code/SKILL.md",
        )
        .await
        .unwrap();

    mock.assert_async().await;
    assert_eq!(blob.sha, "blob-sha");
    assert_eq!(blob.bytes, b"# hi\n");
}

#[tokio::test]
async fn archive_download_extracts_real_root() {
    let mut server = mockito::Server::new_async().await;
    let tarball = test_tarball();
    let mock = server
        .mock("GET", "/api/v5/repos/acme/team-skills/tarball")
        .match_query(Matcher::UrlEncoded("ref".into(), "master".into()))
        .with_status(200)
        .with_header("content-type", "application/gzip")
        .with_body(tarball)
        .create_async()
        .await;
    let provider = GiteeProvider::anonymous(format!("{}/api/v5", server.url())).unwrap();
    let workspace = workspace("acme", "team-skills");
    let dir = tempfile::tempdir().unwrap();
    let mut progress = |_: u64, _: Option<u64>| {};

    let archive = provider
        .download_archive(&workspace, "master", dir.path(), &mut progress)
        .await
        .unwrap();

    mock.assert_async().await;
    assert_eq!(
        archive
            .extracted_root
            .file_name()
            .and_then(|name| name.to_str()),
        Some("repo-root")
    );
    assert!(archive.extracted_root.join("SKILL.md").exists());
}

#[tokio::test]
async fn forbidden_response_maps_to_forbidden_error() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v5/repos/acme/team-skills")
        .with_status(403)
        .with_body(r#"{"message":"403 Forbidden"}"#)
        .create_async()
        .await;
    let provider = GiteeProvider::anonymous(format!("{}/api/v5", server.url())).unwrap();
    let workspace = workspace("acme", "team-skills");

    let err = provider.get_source(&workspace).await.unwrap_err();

    mock.assert_async().await;
    assert!(
        matches!(err, ProviderError::Forbidden { .. }),
        "expected Forbidden, got {err:?}"
    );
}

fn workspace(owner: &str, repo: &str) -> WorkspaceRef {
    WorkspaceRef {
        provider: "gitee.com".to_owned(),
        owner: owner.to_owned(),
        repo: repo.to_owned(),
        remote_id: None,
    }
}

fn repo_json(full_name: &str, private: bool, permissions: &str) -> String {
    let repo = full_name.rsplit('/').next().unwrap_or(full_name);
    format!(
        r#"{{
            "full_name": "{full_name}",
            "path": "{repo}",
            "name": "{repo}",
            "default_branch": "master",
            "private": {private},
            "html_url": "https://gitee.com/{full_name}",
            {permissions}
        }}"#
    )
}

fn test_tarball() -> Vec<u8> {
    let mut tar_data = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_data);
        let bytes = b"# Skill\n";
        let mut header = tar::Header::new_gnu();
        header.set_path("repo-root/SKILL.md").unwrap();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &bytes[..]).unwrap();
        builder.finish().unwrap();
    }
    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&tar_data).unwrap();
    encoder.finish().unwrap()
}
