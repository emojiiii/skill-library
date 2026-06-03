use std::collections::BTreeMap;

use mockito::Matcher;
use skill_library_core::{AuthMode, ProviderCredential, ProviderCredentialMetadata, WorkspaceRef};
use skill_library_provider::{Capability, FileKind, ProviderError, SkillSourceProvider, SourceRef};

use crate::propfind::parse_propfind_response;
use crate::{WebDavAuth, WebDavIndexSkill, WebDavProvider};

fn workspace() -> WorkspaceRef {
    WorkspaceRef {
        provider: "webdav.test".to_owned(),
        owner: "team".to_owned(),
        repo: "skills".to_owned(),
        remote_id: None,
    }
}

fn propfind_body(responses: &[(&str, bool, Option<&str>)]) -> String {
    let mut body = String::from(r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:">"#);
    for (href, collection, etag) in responses {
        body.push_str("<d:response>");
        body.push_str(&format!("<d:href>{href}</d:href><d:propstat><d:prop>"));
        if *collection {
            body.push_str("<d:resourcetype><d:collection/></d:resourcetype>");
        } else {
            body.push_str("<d:resourcetype/>");
        }
        if let Some(etag) = etag {
            body.push_str(&format!("<d:getetag>{etag}</d:getetag>"));
        }
        body.push_str("</d:prop></d:propstat></d:response>");
    }
    body.push_str("</d:multistatus>");
    body
}

#[tokio::test]
async fn list_files_walks_propfind_tree() {
    let mut server = mockito::Server::new_async().await;
    let root = server
        .mock("PROPFIND", "/dav/team/skills/")
        .match_header("depth", "1")
        .with_status(207)
        .with_header("content-type", "application/xml")
        .with_body(propfind_body(&[
            ("/dav/team/skills/", true, None),
            ("/dav/team/skills/code-reviewer/", true, None),
            ("/dav/team/skills/manifest.yaml", false, Some("\"m1\"")),
        ]))
        .create_async()
        .await;
    let nested = server
        .mock("PROPFIND", "/dav/team/skills/code-reviewer/")
        .match_header("depth", "1")
        .with_status(207)
        .with_header("content-type", "application/xml")
        .with_body(propfind_body(&[
            ("/dav/team/skills/code-reviewer/", true, None),
            (
                "/dav/team/skills/code-reviewer/SKILL.md",
                false,
                Some("\"s1\""),
            ),
        ]))
        .create_async()
        .await;
    let provider = WebDavProvider::anonymous(format!("{}/dav", server.url())).unwrap();

    let files = provider
        .list_files(&workspace(), &SourceRef::Latest)
        .await
        .unwrap();

    root.assert_async().await;
    nested.assert_async().await;
    assert_eq!(
        files
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        vec!["code-reviewer", "code-reviewer/SKILL.md", "manifest.yaml"]
    );
    assert!(matches!(files[0].kind, FileKind::Directory));
    assert!(matches!(files[1].kind, FileKind::File));
}

#[tokio::test]
async fn read_file_hashes_when_etag_missing() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/dav/team/skills/SKILL.md")
        .with_status(200)
        .with_body("skill")
        .create_async()
        .await;
    let provider = WebDavProvider::anonymous(format!("{}/dav", server.url())).unwrap();

    let blob = provider
        .read_file(&workspace(), &SourceRef::Latest, "SKILL.md")
        .await
        .unwrap();

    mock.assert_async().await;
    assert_eq!(blob.bytes, b"skill");
    assert!(blob.sha.starts_with("sha256:"));
}

#[tokio::test]
async fn read_file_percent_encodes_paths_with_spaces() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/dav/team/skills/a%20skill/SKILL.md")
        .with_status(200)
        .with_body("skill")
        .create_async()
        .await;
    let provider = WebDavProvider::anonymous(format!("{}/dav", server.url())).unwrap();

    let blob = provider
        .read_file(&workspace(), &SourceRef::Latest, "a skill/SKILL.md")
        .await
        .unwrap();

    mock.assert_async().await;
    assert_eq!(blob.bytes, b"skill");
}

#[tokio::test]
async fn read_file_maps_rate_limit_status() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/dav/team/skills/SKILL.md")
        .with_status(429)
        .with_body("too many requests")
        .create_async()
        .await;
    let provider = WebDavProvider::anonymous(format!("{}/dav", server.url())).unwrap();

    let err = provider
        .read_file(&workspace(), &SourceRef::Latest, "SKILL.md")
        .await
        .unwrap_err();

    mock.assert_async().await;
    assert!(matches!(err, ProviderError::RateLimited { .. }));
}

#[tokio::test]
async fn download_snapshot_uses_index_version_dirs() {
    let mut server = mockito::Server::new_async().await;
    let index = r#"{
            "schemaVersion": 1,
            "skills": [{
                "id": "code-reviewer",
                "latest": "versions/1.1.0",
                "versions": {
                    "1.0.0": "versions/1.0.0",
                    "1.1.0": "versions/1.1.0"
                }
            }]
        }"#;
    let index_mock = server
        .mock("GET", "/dav/team/skills/.skill-library/index.json")
        .with_status(200)
        .with_body(index)
        .create_async()
        .await;
    let propfind = server
        .mock("PROPFIND", "/dav/team/skills/code-reviewer/versions/1.0.0/")
        .match_header("depth", "1")
        .with_status(207)
        .with_header("content-type", "application/xml")
        .with_body(propfind_body(&[
            ("/dav/team/skills/code-reviewer/versions/1.0.0/", true, None),
            (
                "/dav/team/skills/code-reviewer/versions/1.0.0/SKILL.md",
                false,
                Some("\"v1\""),
            ),
        ]))
        .create_async()
        .await;
    let file = server
        .mock(
            "GET",
            "/dav/team/skills/code-reviewer/versions/1.0.0/SKILL.md",
        )
        .with_status(200)
        .with_body("---\nid: code-reviewer\nname: Code Reviewer\n---\n")
        .create_async()
        .await;
    let provider = WebDavProvider::anonymous(format!("{}/dav", server.url())).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut progress = |_: u64, _: Option<u64>| {};

    let archive = provider
        .download_snapshot(
            &workspace(),
            &SourceRef::Version("1.0.0".to_owned()),
            dir.path(),
            &mut progress,
        )
        .await
        .unwrap();

    index_mock.assert_async().await;
    propfind.assert_async().await;
    file.assert_async().await;
    assert!(archive
        .extracted_root
        .join("code-reviewer")
        .join("SKILL.md")
        .exists());
    assert_eq!(archive.ref_name, "1.0.0");
}

#[test]
fn index_skill_resolves_relative_version_paths() {
    let skill = WebDavIndexSkill {
        id: "code-reviewer".to_owned(),
        path: None,
        latest: Some("versions/1.1.0".to_owned()),
        versions: BTreeMap::from([("1.0.0".to_owned(), "versions/1.0.0".to_owned())]),
        checksum: None,
    };

    assert_eq!(skill.display_path(), "code-reviewer");
    assert_eq!(
        skill.dir_for_ref(Some("1.0.0")).as_deref(),
        Some("code-reviewer/versions/1.0.0")
    );
    assert_eq!(
        skill.dir_for_ref(None).as_deref(),
        Some("code-reviewer/versions/1.1.0")
    );
}

#[test]
fn webdav_capabilities_do_not_expose_git_or_social_writes() {
    let provider = WebDavProvider::anonymous("https://example.com/dav").unwrap();
    let caps = provider.capabilities();

    assert_eq!(caps.file_storage, Capability::Supported);
    assert_eq!(caps.versions_index, Capability::Supported);
    assert_eq!(caps.change_requests, Capability::Unsupported);
    assert_eq!(caps.direct_file_write, Capability::Unsupported);
    assert_eq!(caps.discussions, Capability::Unsupported);
    assert_eq!(caps.webhooks, Capability::Unsupported);
}

#[test]
fn basic_credentials_can_use_login_metadata() {
    let credential = ProviderCredential {
        metadata: ProviderCredentialMetadata {
            provider: "webdav.test".to_owned(),
            login: Some("alice".to_owned()),
            scopes: Vec::new(),
            auth_mode: AuthMode::AppPassword,
        },
        token: "app-password".to_owned(),
    };

    assert!(matches!(
        WebDavAuth::from_credential(&credential),
        Some(WebDavAuth::Basic { username, password })
            if username == "alice" && password == "app-password"
    ));
}

#[test]
fn propfind_xml_parser_handles_namespaces() {
    let provider = WebDavProvider::anonymous("https://example.com/dav").unwrap();
    let body = propfind_body(&[
        ("/dav/team/skills/", true, None),
        (
            "/dav/team/skills/a%20skill/SKILL.md",
            false,
            Some("\"etag\""),
        ),
    ]);

    let entries = parse_propfind_response(&provider, "team/skills", &body).unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].relative_path, "");
    assert_eq!(entries[1].relative_path, "a skill/SKILL.md");
    assert_eq!(entries[1].stable_id(), "\"etag\"");
}

#[tokio::test]
async fn bearer_token_auth_sets_authorization_header() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/dav/team/skills/SKILL.md")
        .match_header("authorization", Matcher::Exact("Bearer token".to_owned()))
        .with_status(200)
        .with_body("skill")
        .create_async()
        .await;
    let provider = WebDavProvider::with_instance_base_url(
        "webdav.test",
        format!("{}/dav", server.url()),
        Some(WebDavAuth::Bearer("token".to_owned())),
    )
    .unwrap();

    provider
        .read_file(&workspace(), &SourceRef::Latest, "SKILL.md")
        .await
        .unwrap();

    mock.assert_async().await;
}
