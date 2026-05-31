use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use teamai_core::{RiskLevel, WorkspaceRef};
use teamai_manifest::{effective_risk, parse_skill_dir, SkillManifest};
use walkdir::WalkDir;

pub type Result<T> = std::result::Result<T, PublishError>;

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest error: {0}")]
    Manifest(#[from] teamai_manifest::ManifestError),
    #[error("invalid publish source: {0}")]
    InvalidSource(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPackage {
    pub manifest: SkillManifest,
    pub source_path: PathBuf,
    pub source_hash: String,
    pub risk_level: RiskLevel,
    pub file_count: usize,
    pub total_bytes: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PublishPolicyDecision {
    AllowAutoMerge,
    RequireReview,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishPolicyResult {
    pub decision: PublishPolicyDecision,
    pub schema_passed: bool,
    pub auto_merge_allowed: bool,
    pub reasons: Vec<String>,
    pub risk_level: RiskLevel,
    pub dangerous_permissions: Vec<String>,
    pub script_files: Vec<String>,
    pub large_files: Vec<PublishPolicyFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishPolicyFile {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPolicyOptions {
    #[serde(default = "default_large_file_threshold")]
    pub large_file_threshold_bytes: u64,
}

impl Default for PublishPolicyOptions {
    fn default() -> Self {
        Self {
            large_file_threshold_bytes: default_large_file_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequestSummary {
    pub branch_name: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishFile {
    pub relative_path: PathBuf,
    pub target_path: String,
    pub bytes: Vec<u8>,
}

pub fn package_skill(path: impl AsRef<Path>) -> Result<PublishPackage> {
    let path = path.as_ref();
    let parse_result = parse_skill_dir(path)?;
    let manifest = parse_result
        .manifest
        .ok_or_else(|| PublishError::InvalidSource(format!("{:?}", parse_result.errors)))?;
    let mut hasher = Sha256::new();
    let mut files = Vec::new();
    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry.map_err(|err| PublishError::InvalidSource(err.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(path)
            .map_err(|err| PublishError::InvalidSource(err.to_string()))?;
        if rel.components().any(|component| {
            let value = component.as_os_str().to_string_lossy();
            matches!(value.as_ref(), ".git" | "node_modules" | ".DS_Store")
        }) {
            continue;
        }
        files.push(rel.to_path_buf());
    }
    files.sort();

    let mut total_bytes = 0_u64;
    for rel in &files {
        let full = path.join(rel);
        let bytes = fs::read(&full)?;
        total_bytes += bytes.len() as u64;
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(&bytes);
        hasher.update([0]);
    }

    let risk_level = effective_risk(&manifest);

    Ok(PublishPackage {
        manifest,
        source_path: path.to_path_buf(),
        source_hash: hex::encode(hasher.finalize()),
        risk_level,
        file_count: files.len(),
        total_bytes,
        created_at: Utc::now(),
    })
}

pub fn build_publish_request(
    package: &PublishPackage,
    workspace: &WorkspaceRef,
    source_user: &str,
) -> PublishRequestSummary {
    let short_hash = &package.source_hash[..12.min(package.source_hash.len())];
    let branch_name = format!("teamai/import/{}/{}", package.manifest.id, short_hash);
    let title = format!(
        "Import skill {} v{}",
        package.manifest.name, package.manifest.version
    );
    let body = format!(
        r#"## Team AI Hub Publish

Source-User: {source_user}
Source-Path: {}
Source-Hash: {}
Target-Workspace: {}
Skill-ID: {}
Skill-Version: {}
Risk-Level: {:?}
Files: {}
Bytes: {}

### Policy Check

{}

### Manifest Summary

{}
"#,
        package.source_path.display(),
        package.source_hash,
        workspace.full_name(),
        package.manifest.id,
        package.manifest.version,
        package.risk_level,
        package.file_count,
        package.total_bytes,
        evaluate_publish_policy(package)
            .map(|policy| serde_json::to_string_pretty(&policy).unwrap_or_else(|_| "{}".to_owned()))
            .unwrap_or_else(|err| format!("policy evaluation failed: {err}")),
        serde_json::to_string_pretty(&package.manifest).unwrap_or_else(|_| "{}".to_owned())
    );
    PublishRequestSummary {
        branch_name,
        title,
        body,
    }
}

pub fn collect_publish_files(package: &PublishPackage) -> Result<Vec<PublishFile>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(&package.source_path).follow_links(false) {
        let entry = entry.map_err(|err| PublishError::InvalidSource(err.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&package.source_path)
            .map_err(|err| PublishError::InvalidSource(err.to_string()))?;
        if should_skip(rel) {
            continue;
        }
        validate_relative_path(rel)?;
        let target_path = repo_skill_path(&package.manifest.id, rel)?;
        files.push(PublishFile {
            relative_path: rel.to_path_buf(),
            target_path,
            bytes: fs::read(entry.path())?,
        });
    }
    files.sort_by(|a, b| a.target_path.cmp(&b.target_path));
    Ok(files)
}

pub fn evaluate_publish_policy(package: &PublishPackage) -> Result<PublishPolicyResult> {
    evaluate_publish_policy_with_options(package, PublishPolicyOptions::default())
}

pub fn evaluate_publish_policy_with_options(
    package: &PublishPackage,
    options: PublishPolicyOptions,
) -> Result<PublishPolicyResult> {
    let files = collect_publish_files(package)?;
    let dangerous_permissions = package
        .manifest
        .permissions
        .iter()
        .filter(|permission| {
            matches!(
                permission.as_str(),
                "filesystem.write" | "shell.execute" | "network.external" | "secrets.read"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let script_files = files
        .iter()
        .filter(|file| is_script_file(&file.relative_path))
        .map(|file| rel_path_display(&file.relative_path))
        .collect::<Vec<_>>();
    let large_files = files
        .iter()
        .filter(|file| file.bytes.len() as u64 > options.large_file_threshold_bytes)
        .map(|file| PublishPolicyFile {
            path: rel_path_display(&file.relative_path),
            bytes: file.bytes.len() as u64,
        })
        .collect::<Vec<_>>();

    let mut reasons = Vec::new();
    if package.risk_level != RiskLevel::Low {
        reasons.push(format!("risk level is {}", package.risk_level));
    }
    if !dangerous_permissions.is_empty() {
        reasons.push(format!(
            "dangerous permissions: {}",
            dangerous_permissions.join(", ")
        ));
    }
    if !script_files.is_empty() {
        reasons.push(format!("script files: {}", script_files.join(", ")));
    }
    if !large_files.is_empty() {
        reasons.push(format!("large files: {}", large_files.len()));
    }

    let auto_merge_allowed = reasons.is_empty();
    let decision = if !dangerous_permissions.is_empty() {
        PublishPolicyDecision::Reject
    } else if auto_merge_allowed {
        PublishPolicyDecision::AllowAutoMerge
    } else {
        PublishPolicyDecision::RequireReview
    };

    Ok(PublishPolicyResult {
        decision,
        schema_passed: true,
        auto_merge_allowed,
        reasons,
        risk_level: package.risk_level,
        dangerous_permissions,
        script_files,
        large_files,
    })
}

fn rel_path_display(relative_path: &Path) -> String {
    // Forward-slash join the path components so report/display paths are
    // consistent across platforms (Windows would otherwise show backslashes).
    relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn repo_skill_path(skill_id: &str, relative_path: &Path) -> Result<String> {
    validate_relative_path(relative_path)?;
    let mut parts = vec![skill_id.to_owned()];
    for component in relative_path.components() {
        let Component::Normal(value) = component else {
            return Err(PublishError::InvalidSource(format!(
                "unsafe path component in {}",
                relative_path.display()
            )));
        };
        parts.push(value.to_string_lossy().to_string());
    }
    Ok(parts.join("/"))
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(PublishError::InvalidSource(
            "publish path cannot be empty".to_owned(),
        ));
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(PublishError::InvalidSource(format!(
                "path traversal is not allowed: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn should_skip(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(value.as_ref(), ".git" | "node_modules" | ".DS_Store")
    })
}

fn is_script_file(path: &Path) -> bool {
    let extension = path.extension().and_then(|value| value.to_str());
    let file_name = path.file_name().and_then(|value| value.to_str());
    matches!(
        extension,
        Some("sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd" | "js" | "mjs" | "cjs" | "ts")
    ) || matches!(file_name, Some("Makefile" | "Justfile"))
}

fn default_large_file_threshold() -> u64 {
    1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_hash_is_stable_for_same_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.yaml"),
            r#"
id: local-helper
type: skill
name: Local Helper
description: Helps with local development tasks.
version: 0.1.0
targets:
  - claude-code
permissions:
  - shell.execute.limited
"#,
        )
        .unwrap();
        fs::write(dir.path().join("SKILL.md"), "# Local Helper\n").unwrap();

        let first = package_skill(dir.path()).unwrap();
        let second = package_skill(dir.path()).unwrap();
        assert_eq!(first.source_hash, second.source_hash);
        assert_eq!(first.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn collect_publish_files_targets_skill_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.yaml"),
            r#"
id: local-helper
type: skill
name: Local Helper
description: Helps with local development tasks.
version: 0.1.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(dir.path().join("SKILL.md"), "# Local Helper\n").unwrap();
        fs::create_dir_all(dir.path().join("scripts")).unwrap();
        fs::write(dir.path().join("scripts/run.sh"), "echo ok\n").unwrap();

        let package = package_skill(dir.path()).unwrap();
        let files = collect_publish_files(&package).unwrap();
        let paths = files
            .into_iter()
            .map(|file| file.target_path)
            .collect::<Vec<_>>();
        assert!(paths.contains(&"local-helper/SKILL.md".to_owned()));
        assert!(paths.contains(&"local-helper/manifest.yaml".to_owned()));
        assert!(paths.contains(&"local-helper/scripts/run.sh".to_owned()));
    }

    #[test]
    fn publish_policy_allows_low_risk_without_scripts_or_large_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.yaml"),
            r#"
id: local-helper
type: skill
name: Local Helper
description: Helps with local development tasks.
version: 0.1.0
targets:
  - claude-code
permissions:
  - filesystem.read
"#,
        )
        .unwrap();
        fs::write(dir.path().join("SKILL.md"), "# Local Helper\n").unwrap();

        let package = package_skill(dir.path()).unwrap();
        let policy = evaluate_publish_policy(&package).unwrap();

        assert_eq!(policy.decision, PublishPolicyDecision::AllowAutoMerge);
        assert!(policy.auto_merge_allowed);
        assert!(policy.reasons.is_empty());
    }

    #[test]
    fn publish_policy_rejects_dangerous_permissions_and_flags_scripts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.yaml"),
            r#"
id: local-helper
type: skill
name: Local Helper
description: Helps with local development tasks.
version: 0.1.0
targets:
  - claude-code
permissions:
  - network.external
"#,
        )
        .unwrap();
        fs::write(dir.path().join("SKILL.md"), "# Local Helper\n").unwrap();
        fs::create_dir_all(dir.path().join("scripts")).unwrap();
        fs::write(dir.path().join("scripts/run.sh"), "echo ok\n").unwrap();

        let package = package_skill(dir.path()).unwrap();
        let policy = evaluate_publish_policy(&package).unwrap();

        assert_eq!(policy.decision, PublishPolicyDecision::Reject);
        assert!(!policy.auto_merge_allowed);
        assert_eq!(policy.dangerous_permissions, vec!["network.external"]);
        assert_eq!(policy.script_files, vec!["scripts/run.sh"]);
    }

    #[test]
    fn publish_policy_requires_review_for_large_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.yaml"),
            r#"
id: local-helper
type: skill
name: Local Helper
description: Helps with local development tasks.
version: 0.1.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(dir.path().join("SKILL.md"), "# Local Helper\n").unwrap();
        fs::write(dir.path().join("large.bin"), [0_u8; 256]).unwrap();

        let package = package_skill(dir.path()).unwrap();
        let policy = evaluate_publish_policy_with_options(
            &package,
            PublishPolicyOptions {
                large_file_threshold_bytes: 200,
            },
        )
        .unwrap();

        assert_eq!(policy.decision, PublishPolicyDecision::RequireReview);
        assert_eq!(
            policy.large_files,
            vec![PublishPolicyFile {
                path: "large.bin".to_owned(),
                bytes: 256
            }]
        );
    }
}
