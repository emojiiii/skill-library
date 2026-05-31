use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use teamai_core::{RuntimeTarget, TeamAIError};
use teamai_manifest::{parse_skill_dir, SkillManifest};
use walkdir::WalkDir;

pub type Result<T> = std::result::Result<T, InstallerError>;

#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error("team ai error: {0}")]
    Core(#[from] TeamAIError),
    #[error("manifest error: {0}")]
    Manifest(#[from] teamai_manifest::ManifestError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("install swap failed and previous version was restored: {0}")]
    SwapRestored(String),
    #[error("install swap failed and previous version restore also failed: install={install_error}; restore={restore_error}")]
    SwapRestoreFailed {
        install_error: String,
        restore_error: String,
    },
    #[error("invalid skill source: {0}")]
    InvalidSource(String),
    #[error("unsupported target: {0}")]
    UnsupportedTarget(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallOptions {
    pub source_dir: PathBuf,
    pub targets: Vec<String>,
    #[serde(default)]
    pub target_roots: Vec<TargetRoot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetRoot {
    pub target: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    pub manifest: SkillManifest,
    pub installed: Vec<TargetInstallReport>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetInstallReport {
    pub target: String,
    pub path: PathBuf,
    pub installed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub installed_at: DateTime<Utc>,
    pub source: PathBuf,
    pub target: String,
    pub managed_by: String,
}

pub fn install(options: InstallOptions) -> Result<InstallReport> {
    let parse_result = parse_skill_dir(&options.source_dir)?;
    let manifest = parse_result
        .manifest
        .ok_or_else(|| InstallerError::InvalidSource(format!("{:?}", parse_result.errors)))?;
    let target_roots = TargetRoots::new(options.target_roots);
    // An explicit empty target list means "download/stage only, deploy nowhere"
    // — the caller wants the skill source resolved but not linked into any agent
    // directory. We therefore install to exactly the requested targets, with no
    // "empty means all" fallback. (Callers that want every declared target pass
    // them explicitly.)
    let requested = options.targets.clone();

    let mut installed = Vec::new();
    let mut skipped = Vec::new();
    for target in requested {
        if !manifest.targets.contains(&target) {
            skipped.push(format!("{target}: not declared by manifest"));
            continue;
        }
        let root = match target_roots.resolve(&target) {
            Some(root) => root,
            None => {
                skipped.push(format!("{target}: unsupported target"));
                continue;
            }
        };
        fs::create_dir_all(&root)?;
        let final_dir = root.join(&manifest.id);
        let temp_parent = root.join(".teamai-tmp");
        fs::create_dir_all(&temp_parent)?;
        let staging = tempfile::Builder::new()
            .prefix(&manifest.id)
            .tempdir_in(&temp_parent)?;
        copy_skill_files(&options.source_dir, staging.path())?;
        let metadata = InstallMetadata {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            installed_at: Utc::now(),
            source: options.source_dir.clone(),
            target: target.clone(),
            managed_by: "team-ai-hub".to_owned(),
        };
        fs::write(
            staging.path().join(".teamai-install.json"),
            serde_json::to_vec_pretty(&metadata)
                .map_err(|err| InstallerError::InvalidSource(err.to_string()))?,
        )?;
        if final_dir.exists() {
            let backup = root.join(format!(".{}.teamai-backup", manifest.id));
            replace_existing_install(&final_dir, staging.path(), &backup)?;
        } else {
            fs::rename(staging.path(), &final_dir)?;
        }
        installed.push(TargetInstallReport {
            target,
            path: final_dir,
            installed_at: metadata.installed_at,
        });
    }

    Ok(InstallReport {
        manifest,
        installed,
        skipped,
    })
}

pub fn remove(skill_id: &str, targets: &[String], roots: Vec<TargetRoot>) -> Result<Vec<PathBuf>> {
    let target_roots = TargetRoots::new(roots);
    let mut removed = Vec::new();
    for target in targets {
        if let Some(root) = target_roots.resolve(target) {
            let path = root.join(skill_id);
            if path.exists() {
                fs::remove_dir_all(&path)?;
                removed.push(path);
            }
        }
    }
    Ok(removed)
}

pub fn list_installed(target: &str, roots: Vec<TargetRoot>) -> Result<Vec<InstallMetadata>> {
    let Some(root) = TargetRoots::new(roots).resolve(target) else {
        return Err(InstallerError::UnsupportedTarget(target.to_owned()));
    };
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut installed = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let metadata = entry.path().join(".teamai-install.json");
        if metadata.exists() {
            let raw = fs::read_to_string(metadata)?;
            if let Ok(value) = serde_json::from_str::<InstallMetadata>(&raw) {
                installed.push(value);
            }
        }
    }
    installed.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(installed)
}

fn copy_skill_files(source: &Path, dest: &Path) -> Result<()> {
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|err| InstallerError::InvalidSource(err.to_string()))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(source)
            .map_err(|err| InstallerError::InvalidSource(err.to_string()))?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        validate_relative_path(rel)?;
        if should_exclude(rel) {
            continue;
        }
        let dest_path = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &dest_path)?;
            let permissions = fs::metadata(path)?.permissions();
            fs::set_permissions(&dest_path, permissions)?;
        }
    }
    if !dest.join("SKILL.md").exists() {
        return Err(InstallerError::InvalidSource(
            "SKILL.md was not copied into install payload".to_owned(),
        ));
    }
    Ok(())
}

fn replace_existing_install(final_dir: &Path, staging_dir: &Path, backup_dir: &Path) -> Result<()> {
    if backup_dir.exists() {
        fs::remove_dir_all(backup_dir)?;
    }
    fs::rename(final_dir, backup_dir)?;
    match rename_staging_into_place(staging_dir, final_dir) {
        Ok(()) => {
            fs::remove_dir_all(backup_dir)?;
            Ok(())
        }
        Err(install_error) => {
            let install_message = install_error.to_string();
            match restore_backup(final_dir, backup_dir) {
                Ok(()) => Err(InstallerError::SwapRestored(install_message)),
                Err(restore_error) => Err(InstallerError::SwapRestoreFailed {
                    install_error: install_message,
                    restore_error: restore_error.to_string(),
                }),
            }
        }
    }
}

fn rename_staging_into_place(staging_dir: &Path, final_dir: &Path) -> std::io::Result<()> {
    if std::env::var_os("TEAMAI_INSTALLER_INJECT_SWAP_FAILURE").is_some()
        && staging_dir.join(".teamai-fail-swap").exists()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "injected install swap failure",
        ));
    }
    fs::rename(staging_dir, final_dir)
}

fn restore_backup(final_dir: &Path, backup_dir: &Path) -> std::io::Result<()> {
    if final_dir.exists() {
        fs::remove_dir_all(final_dir)?;
    }
    fs::rename(backup_dir, final_dir)
}

fn validate_relative_path(path: &Path) -> Result<()> {
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(InstallerError::InvalidSource(format!(
                "path traversal is not allowed: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn should_exclude(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(
            value.as_ref(),
            ".git" | "node_modules" | ".DS_Store" | ".teamai-install.json"
        )
    })
}

struct TargetRoots {
    overrides: Vec<TargetRoot>,
}

impl TargetRoots {
    fn new(overrides: Vec<TargetRoot>) -> Self {
        Self { overrides }
    }

    fn resolve(&self, target: &str) -> Option<PathBuf> {
        if let Some(root) = self
            .overrides
            .iter()
            .find(|override_root| override_root.target == target)
        {
            return Some(root.root.clone());
        }
        let home = dirs::home_dir()?;
        match RuntimeTarget::from_id(target) {
            RuntimeTarget::ClaudeCode => Some(home.join(".claude").join("skills")),
            RuntimeTarget::Cursor => Some(home.join(".cursor").join("skills")),
            RuntimeTarget::Codex => Some(home.join(".codex").join("skills")),
            RuntimeTarget::Custom(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SwapFailureInjection;

    impl SwapFailureInjection {
        fn enable() -> Self {
            std::env::set_var("TEAMAI_INSTALLER_INJECT_SWAP_FAILURE", "1");
            Self
        }
    }

    impl Drop for SwapFailureInjection {
        fn drop(&mut self) {
            std::env::remove_var("TEAMAI_INSTALLER_INJECT_SWAP_FAILURE");
        }
    }

    #[test]
    fn installs_skill_atomically_to_requested_target() {
        let source = tempfile::tempdir().unwrap();
        fs::write(
            source.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness.
version: 1.0.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(source.path().join("SKILL.md"), "# Code Reviewer\n").unwrap();
        let target = tempfile::tempdir().unwrap();
        let report = install(InstallOptions {
            source_dir: source.path().to_path_buf(),
            targets: vec!["claude-code".to_owned()],
            target_roots: vec![TargetRoot {
                target: "claude-code".to_owned(),
                root: target.path().to_path_buf(),
            }],
        })
        .unwrap();

        assert_eq!(report.installed.len(), 1);
        assert!(target.path().join("code-reviewer/SKILL.md").exists());
        assert!(target
            .path()
            .join("code-reviewer/.teamai-install.json")
            .exists());
    }

    #[test]
    fn empty_targets_deploys_nowhere() {
        // An explicit empty target list means "download/stage only" — nothing
        // should be linked into any agent directory (no "empty means all").
        let source = tempfile::tempdir().unwrap();
        fs::write(
            source.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness.
version: 1.0.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(source.path().join("SKILL.md"), "# Code Reviewer\n").unwrap();
        let target = tempfile::tempdir().unwrap();
        let report = install(InstallOptions {
            source_dir: source.path().to_path_buf(),
            targets: Vec::new(),
            target_roots: vec![TargetRoot {
                target: "claude-code".to_owned(),
                root: target.path().to_path_buf(),
            }],
        })
        .unwrap();

        assert!(report.installed.is_empty(), "nothing should be deployed");
        assert!(
            !target.path().join("code-reviewer").exists(),
            "no skill dir should be created in any target root"
        );
    }

    #[test]
    fn restores_previous_install_when_swap_fails() {
        let source_v1 = tempfile::tempdir().unwrap();
        fs::write(
            source_v1.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 1.0.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(source_v1.path().join("SKILL.md"), "# Code Reviewer v1\n").unwrap();
        let target = tempfile::tempdir().unwrap();
        install(InstallOptions {
            source_dir: source_v1.path().to_path_buf(),
            targets: vec!["claude-code".to_owned()],
            target_roots: vec![TargetRoot {
                target: "claude-code".to_owned(),
                root: target.path().to_path_buf(),
            }],
        })
        .unwrap();

        let source_v2 = tempfile::tempdir().unwrap();
        fs::write(
            source_v2.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 2.0.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(source_v2.path().join("SKILL.md"), "# Code Reviewer v2\n").unwrap();
        fs::write(source_v2.path().join(".teamai-fail-swap"), "fail\n").unwrap();
        let _injection = SwapFailureInjection::enable();

        let err = install(InstallOptions {
            source_dir: source_v2.path().to_path_buf(),
            targets: vec!["claude-code".to_owned()],
            target_roots: vec![TargetRoot {
                target: "claude-code".to_owned(),
                root: target.path().to_path_buf(),
            }],
        })
        .unwrap_err();

        assert!(matches!(err, InstallerError::SwapRestored(_)));
        let installed_skill =
            fs::read_to_string(target.path().join("code-reviewer/SKILL.md")).unwrap();
        assert_eq!(installed_skill, "# Code Reviewer v1\n");
        let metadata =
            fs::read_to_string(target.path().join("code-reviewer/.teamai-install.json")).unwrap();
        assert!(metadata.contains("\"version\": \"1.0.0\""));
    }
}
