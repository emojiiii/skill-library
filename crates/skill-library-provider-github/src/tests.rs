use flate2::write::GzEncoder;
use flate2::Compression;
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    GitRef, Member, PermissionLevel, ProviderError, SkillSourceProvider, WebhookConfig,
};
use std::io::Write;

use crate::models::CollaboratorResponse;
use crate::util::{extract_tarball, github_webhook_request};
use crate::GitHubProvider;

#[test]
fn github_webhook_request_defaults_to_push_and_dedupes_events() {
    let request = github_webhook_request(WebhookConfig {
        events: vec!["push".to_owned(), "release".to_owned(), "push".to_owned()],
        callback_url: "https://team.example/api/webhooks/github".to_owned(),
        secret: "secret".to_owned(),
    })
    .unwrap();
    let value = serde_json::to_value(&request).unwrap();

    assert_eq!(value["name"], "web");
    assert_eq!(value["active"], true);
    assert_eq!(value["events"], serde_json::json!(["push", "release"]));
    assert_eq!(
        value["config"],
        serde_json::json!({
            "url": "https://team.example/api/webhooks/github",
            "content_type": "json",
            "secret": "secret",
            "insecure_ssl": "0"
        })
    );

    let defaulted = github_webhook_request(WebhookConfig {
        events: Vec::new(),
        callback_url: "https://team.example/api/webhooks/github".to_owned(),
        secret: "secret".to_owned(),
    })
    .unwrap();
    assert_eq!(defaulted.events, vec!["push"]);
}

#[test]
fn github_provider_reports_default_instance_id() {
    let provider = GitHubProvider::anonymous("https://api.github.com").unwrap();
    assert_eq!(SkillSourceProvider::id(&provider), "github.com");
}

#[test]
fn github_webhook_request_requires_callback_and_secret() {
    assert!(github_webhook_request(WebhookConfig {
        events: Vec::new(),
        callback_url: String::new(),
        secret: "secret".to_owned(),
    })
    .is_err());
    assert!(github_webhook_request(WebhookConfig {
        events: Vec::new(),
        callback_url: "https://team.example/api/webhooks/github".to_owned(),
        secret: String::new(),
    })
    .is_err());
}

#[test]
fn extract_tarball_skips_pax_global_header_for_top_level() {
    let mut tar_buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_buf);

        let pax_payload = b"52 comment=387020e0000000000000000000000000000000\n";
        let mut pax_header = tar::Header::new_ustar();
        pax_header.set_size(pax_payload.len() as u64);
        pax_header.set_entry_type(tar::EntryType::new(b'g'));
        pax_header.set_cksum();
        builder
            .append_data(&mut pax_header, "pax_global_header", &pax_payload[..])
            .unwrap();

        let skill_md = b"---\nid: nested-skill\ntype: skill\nname: Nested\ndescription: A nested skill.\nversion: 0.1.0\ntargets:\n  - claude-code\n---\n# Nested\n";
        let mut md_header = tar::Header::new_ustar();
        md_header.set_size(skill_md.len() as u64);
        md_header.set_entry_type(tar::EntryType::Regular);
        md_header.set_mode(0o644);
        md_header.set_cksum();
        builder
            .append_data(
                &mut md_header,
                "owner-repo-387020e/skills/cat/nested-skill/SKILL.md",
                &skill_md[..],
            )
            .unwrap();
        builder.finish().unwrap();
    }

    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&tar_buf).unwrap();
    let gz_bytes = gz.finish().unwrap();

    let dest = tempfile::tempdir().unwrap();
    let extracted_root = extract_tarball(&gz_bytes, dest.path()).unwrap();

    assert_eq!(
        extracted_root.file_name().and_then(|n| n.to_str()),
        Some("owner-repo-387020e"),
        "extracted_root should be the real repo dir, got {extracted_root:?}"
    );
    assert!(
        extracted_root.is_dir(),
        "extracted_root must exist as a dir"
    );
    assert!(
        extracted_root
            .join("skills/cat/nested-skill/SKILL.md")
            .exists(),
        "nested skill must be readable under extracted_root"
    );
}

#[test]
fn collaborator_response_maps_highest_permission_to_member_role() {
    let collaborator: CollaboratorResponse = serde_json::from_value(serde_json::json!({
        "login": "octocat",
        "avatar_url": "https://avatars.githubusercontent.com/u/1?v=4",
        "permissions": {
            "admin": false,
            "maintain": true,
            "push": true,
            "triage": true,
            "pull": true
        }
    }))
    .unwrap();

    let member = Member::from(collaborator);

    assert_eq!(member.login, "octocat");
    assert_eq!(member.role, PermissionLevel::Maintain);
    assert_eq!(
        member.avatar_url.as_deref(),
        Some("https://avatars.githubusercontent.com/u/1?v=4")
    );
}

#[test]
fn collaborator_response_without_permissions_maps_to_none() {
    let collaborator: CollaboratorResponse =
        serde_json::from_value(serde_json::json!({ "login": "outside-user" })).unwrap();

    let member = Member::from(collaborator);

    assert_eq!(member.login, "outside-user");
    assert_eq!(member.role, PermissionLevel::None);
    assert_eq!(member.avatar_url, None);
}

#[tokio::test]
async fn deserialize_failure_includes_endpoint_context() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/repos/acme/team-skills/hooks")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"config":{"url":"https://example.com"}}"#)
        .create_async()
        .await;

    let provider = GitHubProvider::anonymous(server.url()).unwrap();
    let workspace = WorkspaceRef::github("acme", "team-skills");

    let result = provider
        .create_webhook(
            &workspace,
            WebhookConfig {
                events: vec!["push".to_owned()],
                callback_url: "https://example.com/hook".to_owned(),
                secret: "shh".to_owned(),
            },
        )
        .await;

    mock.assert_async().await;
    let err = result.expect_err("expected deserialize failure to surface as ProviderError");
    let message = err.to_string();
    assert!(
        message.contains("POST"),
        "error should mention the HTTP method, got: {message}"
    );
    assert!(
        message.contains("/repos/acme/team-skills/hooks"),
        "error should mention the failing path, got: {message}"
    );
    assert!(
        message.contains("missing field"),
        "error should still expose the underlying serde reason, got: {message}"
    );
}

#[tokio::test]
async fn forbidden_response_maps_to_forbidden_error_with_path() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/repos/acme/team-skills/hooks")
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message":"Resource not accessible by integration"}"#)
        .create_async()
        .await;

    let provider = GitHubProvider::anonymous(server.url()).unwrap();
    let workspace = WorkspaceRef::github("acme", "team-skills");

    let result = provider
        .create_webhook(
            &workspace,
            WebhookConfig {
                events: vec!["push".to_owned()],
                callback_url: "https://example.com/hook".to_owned(),
                secret: "shh".to_owned(),
            },
        )
        .await;

    mock.assert_async().await;
    let err = result.expect_err("expected 403 to surface as Forbidden");
    assert!(
        matches!(err, ProviderError::Forbidden { .. }),
        "expected Forbidden, got: {err:?}"
    );
}

#[tokio::test]
async fn github_rate_limit_response_maps_to_rate_limited() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/repos/acme/team-skills/hooks")
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message":"API rate limit exceeded for 203.0.113.1."}"#)
        .create_async()
        .await;

    let provider = GitHubProvider::anonymous(server.url()).unwrap();
    let workspace = WorkspaceRef::github("acme", "team-skills");

    let result = provider
        .create_webhook(
            &workspace,
            WebhookConfig {
                events: vec!["push".to_owned()],
                callback_url: "https://example.com/hook".to_owned(),
                secret: "shh".to_owned(),
            },
        )
        .await;

    mock.assert_async().await;
    let err = result.expect_err("expected GitHub API rate limit to surface as RateLimited");
    assert!(
        matches!(err, ProviderError::RateLimited { .. }),
        "expected RateLimited, got: {err:?}"
    );
}

#[tokio::test]
async fn scan_uses_two_calls_for_any_skill_count() {
    use crate::scan::scan_skill_assets_at;

    let mut server = mockito::Server::new_async().await;

    let tree_mock = server
        .mock("GET", "/repos/acme/team-skills/git/trees/main?recursive=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "tree": [
                    {"path": "skills/code-reviewer/manifest.yaml", "type": "blob", "sha": "a"},
                    {"path": "skills/pr-summarizer/SKILL.md", "type": "blob", "sha": "b"},
                    {"path": "deep/nested/skills/security-auditor/manifest.json", "type": "blob", "sha": "c"},
                    {"path": "README.md", "type": "blob", "sha": "d"},
                    {"path": ".github/workflows/ci.yml", "type": "blob", "sha": "e"}
                ]
            }"#,
        )
        .expect(1)
        .create_async()
        .await;

    let graphql_mock = server
        .mock("POST", "/graphql")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "data": {
                    "repository": {
                        "f0": {"text": "{\"schemaVersion\":1,\"id\":\"security-auditor\",\"type\":\"skill\",\"name\":\"Auditor\",\"description\":\"Audit\",\"version\":\"2.0.0\",\"targets\":[\"claude-code\"]}", "isBinary": false, "byteSize": 150},
                        "f1": {"text": "schemaVersion: 1\nid: code-reviewer\ntype: skill\nname: Code Reviewer\ndescription: Reviews code\nversion: 1.0.0\ntargets: [claude-code]", "isBinary": false, "byteSize": 100},
                        "f2": {"text": "---\nschemaVersion: 1\nid: pr-summarizer\ntype: skill\nname: PR Summarizer\ndescription: Summarizes PRs\nversion: 0.1.0\ntargets: [claude-code]\n---\n# PR Summarizer\n", "isBinary": false, "byteSize": 200}
                    }
                }
            }"#,
        )
        .expect(1)
        .create_async()
        .await;

    let provider = GitHubProvider::anonymous(server.url()).unwrap();
    let workspace = WorkspaceRef::github("acme", "team-skills");
    let skills = scan_skill_assets_at(&provider, &workspace, &GitRef::Branch("main".to_owned()))
        .await
        .expect("scan succeeded");

    tree_mock.assert_async().await;
    graphql_mock.assert_async().await;

    let ids: Vec<&str> = skills.iter().map(|s| s.manifest.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["code-reviewer", "pr-summarizer", "security-auditor"]
    );
}

#[tokio::test]
async fn scan_skips_individual_bad_manifests() {
    use crate::scan::scan_skill_assets_at;

    let mut server = mockito::Server::new_async().await;

    let _tree = server
        .mock("GET", "/repos/acme/team-skills/git/trees/main?recursive=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "tree": [
                    {"path": "good/manifest.yaml", "type": "blob", "sha": "a"},
                    {"path": "bad/manifest.yaml", "type": "blob", "sha": "b"}
                ]
            }"#,
        )
        .create_async()
        .await;

    let _gql = server
        .mock("POST", "/graphql")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "data": {
                    "repository": {
                        "f0": {"text": "schemaVersion: 1\nid: good-skill\ntype: skill\nname: Good\ndescription: ok\nversion: 1.0.0\ntargets: [claude-code]", "isBinary": false, "byteSize": 100},
                        "f1": {"text": "name: Missing required fields\nversion: 0.1.0", "isBinary": false, "byteSize": 50}
                    }
                }
            }"#,
        )
        .create_async()
        .await;

    let provider = GitHubProvider::anonymous(server.url()).unwrap();
    let workspace = WorkspaceRef::github("acme", "team-skills");
    let skills = scan_skill_assets_at(&provider, &workspace, &GitRef::Branch("main".to_owned()))
        .await
        .expect("scan succeeded despite one bad manifest");

    let ids: Vec<&str> = skills.iter().map(|s| s.manifest.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["good-skill"],
        "bad manifest must be skipped, not crash the scan"
    );
}

#[tokio::test]
async fn scan_empty_workspace_skips_graphql() {
    use crate::scan::scan_skill_assets_at;

    let mut server = mockito::Server::new_async().await;

    let _tree = server
        .mock("GET", "/repos/acme/team-skills/git/trees/main?recursive=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{ "tree": [{"path": "README.md", "type": "blob", "sha": "a"}] }"#)
        .create_async()
        .await;

    let no_graphql = server
        .mock("POST", "/graphql")
        .expect(0)
        .create_async()
        .await;

    let provider = GitHubProvider::anonymous(server.url()).unwrap();
    let workspace = WorkspaceRef::github("acme", "team-skills");
    let skills = scan_skill_assets_at(&provider, &workspace, &GitRef::Branch("main".to_owned()))
        .await
        .expect("empty scan should succeed");

    no_graphql.assert_async().await;
    assert!(skills.is_empty());
}
