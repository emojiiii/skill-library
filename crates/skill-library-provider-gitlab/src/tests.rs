use flate2::write::GzEncoder;
use mockito::Matcher;
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ArchiveProvider, FileKind, GitRef, PermissionLevel, ProviderError, SkillSourceProvider,
    SourceRef,
};
use std::io::Write;

use crate::GitLabProvider;

#[test]
fn gitlab_provider_reports_instance_id() {
    let provider =
        GitLabProvider::with_instance_base_url("gitlab.internal", "https://gitlab/api/v4", None)
            .unwrap();

    assert_eq!(SkillSourceProvider::id(&provider), "gitlab.internal");
}

#[tokio::test]
async fn token_auth_uses_private_token_header_only() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v4/projects/group%2Fteam-skills")
        .match_header("private-token", "pat-token")
        .match_header("authorization", Matcher::Missing)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(project_json(
            7,
            "team-skills",
            "group/team-skills",
            Some(30),
        ))
        .create_async()
        .await;
    let provider = GitLabProvider::with_instance_base_url(
        "gitlab.com",
        format!("{}/api/v4", server.url()),
        Some("pat-token".to_owned()),
    )
    .unwrap();
    let workspace = workspace("group", "team-skills");

    let source = provider.get_source(&workspace).await.unwrap();

    mock.assert_async().await;
    assert_eq!(source.full_name, "group/team-skills");
}

#[tokio::test]
async fn validate_token_reads_current_user() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v4/user")
        .match_header("private-token", "pat-token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":12,"username":"dev-user","name":"Dev User"}"#)
        .create_async()
        .await;
    let provider = GitLabProvider::with_instance_base_url(
        "gitlab.com",
        format!("{}/api/v4", server.url()),
        Some("pat-token".to_owned()),
    )
    .unwrap();

    let info = provider.validate_token().await.unwrap();

    mock.assert_async().await;
    assert_eq!(info.login, "dev-user");
}

#[tokio::test]
async fn get_source_encodes_nested_namespace() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v4/projects/group%2Fsubgroup%2Fteam-skills")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(project_json(
            42,
            "team-skills",
            "group/subgroup/team-skills",
            Some(30),
        ))
        .create_async()
        .await;
    let provider = GitLabProvider::anonymous(format!("{}/api/v4", server.url())).unwrap();
    let workspace = workspace("group/subgroup", "team-skills");

    let source = provider.get_source(&workspace).await.unwrap();

    mock.assert_async().await;
    assert_eq!(source.provider, "gitlab.com");
    assert_eq!(source.owner, "group/subgroup");
    assert_eq!(source.repo, "team-skills");
    assert_eq!(source.permission, PermissionLevel::Write);
}

#[tokio::test]
async fn list_files_maps_recursive_tree_entries() {
    let mut server = mockito::Server::new_async().await;
    let project = server
        .mock("GET", "/api/v4/projects/group%2Fteam-skills")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(project_json(
            7,
            "team-skills",
            "group/team-skills",
            Some(40),
        ))
        .create_async()
        .await;
    let tree = server
        .mock(
            "GET",
            "/api/v4/projects/group%2Fteam-skills/repository/tree",
        )
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("recursive".into(), "true".into()),
            Matcher::UrlEncoded("ref".into(), "main".into()),
            Matcher::UrlEncoded("per_page".into(), "100".into()),
            Matcher::UrlEncoded("page".into(), "1".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"id":"a","path":"skills/code/SKILL.md","type":"blob"},
                {"id":"b","path":"skills","type":"tree"},
                {"id":"c","path":"vendor/lib","type":"commit"}
            ]"#,
        )
        .create_async()
        .await;
    let provider = GitLabProvider::anonymous(format!("{}/api/v4", server.url())).unwrap();
    let workspace = workspace("group", "team-skills");

    let files = provider
        .list_files(&workspace, &SourceRef::Latest)
        .await
        .unwrap();

    project.assert_async().await;
    tree.assert_async().await;
    assert_eq!(files.len(), 3);
    assert!(matches!(files[0].kind, FileKind::File));
    assert!(matches!(files[1].kind, FileKind::Directory));
    assert!(matches!(files[2].kind, FileKind::Submodule));
}

#[tokio::test]
async fn read_file_reads_raw_bytes() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock(
            "GET",
            "/api/v4/projects/group%2Fteam-skills/repository/files/skills%2Fcode%2FSKILL.md/raw",
        )
        .match_query(Matcher::UrlEncoded("ref".into(), "main".into()))
        .with_status(200)
        .with_header("x-gitlab-blob-id", "blob-sha")
        .with_body(Vec::from(&b"\0skill"[..]))
        .create_async()
        .await;
    let provider = GitLabProvider::anonymous(format!("{}/api/v4", server.url())).unwrap();
    let workspace = workspace("group", "team-skills");

    let blob = provider
        .read_file(
            &workspace,
            &SourceRef::Git(GitRef::Branch("main".to_owned())),
            "skills/code/SKILL.md",
        )
        .await
        .unwrap();

    mock.assert_async().await;
    assert_eq!(blob.sha, "blob-sha");
    assert_eq!(blob.bytes, b"\0skill");
}

#[tokio::test]
async fn archive_download_extracts_real_root() {
    let mut server = mockito::Server::new_async().await;
    let tarball = test_tarball();
    let mock = server
        .mock(
            "GET",
            "/api/v4/projects/group%2Fteam-skills/repository/archive.tar.gz",
        )
        .match_query(Matcher::UrlEncoded("sha".into(), "main".into()))
        .with_status(200)
        .with_header("content-type", "application/gzip")
        .with_body(tarball)
        .create_async()
        .await;
    let provider = GitLabProvider::anonymous(format!("{}/api/v4", server.url())).unwrap();
    let workspace = workspace("group", "team-skills");
    let dir = tempfile::tempdir().unwrap();
    let mut progress = |_: u64, _: Option<u64>| {};

    let archive = provider
        .download_archive(&workspace, "main", dir.path(), &mut progress)
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
        .mock("GET", "/api/v4/projects/group%2Fteam-skills")
        .with_status(403)
        .with_body(r#"{"message":"403 Forbidden"}"#)
        .create_async()
        .await;
    let provider = GitLabProvider::anonymous(format!("{}/api/v4", server.url())).unwrap();
    let workspace = workspace("group", "team-skills");

    let err = provider.get_source(&workspace).await.unwrap_err();

    mock.assert_async().await;
    assert!(
        matches!(err, ProviderError::Forbidden { .. }),
        "expected Forbidden, got {err:?}"
    );
}

fn workspace(owner: &str, repo: &str) -> WorkspaceRef {
    WorkspaceRef {
        provider: "gitlab.com".to_owned(),
        owner: owner.to_owned(),
        repo: repo.to_owned(),
        remote_id: None,
    }
}

fn project_json(
    id: u64,
    path: &str,
    path_with_namespace: &str,
    access_level: Option<u32>,
) -> String {
    let permissions = access_level
        .map(|level| {
            format!(
                r#""permissions":{{"project_access":{{"access_level":{level}}},"group_access":null}}"#
            )
        })
        .unwrap_or_else(|| r#""permissions":null"#.to_owned());
    format!(
        r#"{{
            "id": {id},
            "path": "{path}",
            "path_with_namespace": "{path_with_namespace}",
            "default_branch": "main",
            "visibility": "private",
            "web_url": "https://gitlab.example/{path_with_namespace}",
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
