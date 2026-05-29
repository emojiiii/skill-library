use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use teamai_core::{risk_for_permissions, RiskLevel};

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ManifestError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub id: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<RepositoryInfo>,
    #[serde(default)]
    pub authors: Vec<Author>,
    pub targets: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub runtime: BTreeMap<String, Value>,
    #[serde(default)]
    pub compatibility: BTreeMap<String, Value>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Value>,
    #[serde(default)]
    pub files: Option<FileRules>,
    #[serde(default)]
    pub risk: Option<RiskInfo>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryInfo {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Author {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileRules {
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_entry() -> String {
    "SKILL.md".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskInfo {
    pub level: RiskLevel,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestIssue {
    pub field: String,
    pub message: String,
}

impl ManifestIssue {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestParseResult {
    pub manifest: Option<SkillManifest>,
    pub errors: Vec<ManifestIssue>,
    pub warnings: Vec<ManifestIssue>,
    pub source: Option<PathBuf>,
}

impl ManifestParseResult {
    pub fn ok(&self) -> bool {
        self.errors.is_empty() && self.manifest.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillAsset {
    pub path: PathBuf,
    pub manifest: SkillManifest,
    pub warnings: Vec<ManifestIssue>,
}

pub fn effective_risk(manifest: &SkillManifest) -> RiskLevel {
    let permission_risk = risk_for_permissions(manifest.permissions.iter().map(String::as_str));
    manifest
        .risk
        .as_ref()
        .map(|risk| risk.level.max(permission_risk))
        .unwrap_or(permission_risk)
}

pub fn parse_skill_dir(path: impl AsRef<Path>) -> Result<ManifestParseResult> {
    let path = path.as_ref();
    let manifest_source = find_manifest_file(path);
    let skill_md_path = path.join("SKILL.md");
    let mut warnings = Vec::new();

    let frontmatter = if skill_md_path.exists() {
        parse_skill_frontmatter(&skill_md_path)?
    } else {
        None
    };

    let (mut value, source) = match manifest_source {
        Some(source) => {
            let raw = fs::read_to_string(&source)?;
            let parsed = parse_manifest_value(&source, &raw)?;
            if let Some(frontmatter) = frontmatter {
                let mut merged = parsed;
                merge_frontmatter_defaults(&mut warnings, &mut merged, &frontmatter);
                (merged, Some(source))
            } else {
                (parsed, Some(source))
            }
        }
        None => match frontmatter {
            Some(frontmatter) => (frontmatter, Some(skill_md_path.clone())),
            None => {
                return Ok(ManifestParseResult {
                    manifest: None,
                    errors: vec![ManifestIssue::new(
                    "manifest",
                    "missing manifest.yaml, manifest.yml, manifest.json, or SKILL.md frontmatter",
                )],
                    warnings,
                    source: None,
                })
            }
        },
    };

    normalize_manifest_value(&mut value);
    let manifest = serde_json::from_value::<SkillManifest>(value.clone())?;
    let errors = validate_manifest(&manifest);
    warnings.extend(warnings_for_manifest(&manifest));

    Ok(ManifestParseResult {
        manifest: errors.is_empty().then_some(manifest),
        errors,
        warnings,
        source,
    })
}

pub fn scan_workspace(path: impl AsRef<Path>) -> Result<Vec<SkillAsset>> {
    let root = path.as_ref();
    let mut assets = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let result = parse_skill_dir(&path)?;
        if let Some(manifest) = result.manifest {
            assets.push(SkillAsset {
                path: path
                    .strip_prefix(root)
                    .unwrap_or(path.as_path())
                    .to_path_buf(),
                manifest,
                warnings: result.warnings,
            });
        }
    }

    assets.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    Ok(assets)
}

pub fn semantic_diff(from: &SkillManifest, to: &SkillManifest) -> Vec<SemanticChange> {
    let mut changes = Vec::new();
    if from.version != to.version {
        changes.push(SemanticChange::changed(
            "version",
            Value::String(from.version.clone()),
            Value::String(to.version.clone()),
        ));
    }
    diff_string_set(
        "permissions",
        &from.permissions,
        &to.permissions,
        &mut changes,
    );
    diff_string_set("targets", &from.targets, &to.targets, &mut changes);

    if from.dependencies != to.dependencies {
        changes.push(SemanticChange::changed(
            "dependencies",
            serde_json::to_value(&from.dependencies).unwrap_or(Value::Null),
            serde_json::to_value(&to.dependencies).unwrap_or(Value::Null),
        ));
    }
    if from.files != to.files {
        changes.push(SemanticChange::changed(
            "files",
            serde_json::to_value(&from.files).unwrap_or(Value::Null),
            serde_json::to_value(&to.files).unwrap_or(Value::Null),
        ));
    }
    changes
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticChange {
    pub path: String,
    pub kind: SemanticChangeKind,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub before: Option<Value>,
    #[serde(default)]
    pub after: Option<Value>,
    #[serde(default)]
    pub risk: Option<RiskLevel>,
}

impl SemanticChange {
    fn added(path: impl Into<String>, value: Value) -> Self {
        let path = path.into();
        let risk = value
            .as_str()
            .filter(|_| path.starts_with("permissions"))
            .map(|permission| risk_for_permissions([permission]));
        Self {
            path,
            kind: SemanticChangeKind::Added,
            value: Some(value),
            before: None,
            after: None,
            risk,
        }
    }

    fn removed(path: impl Into<String>, value: Value) -> Self {
        Self {
            path: path.into(),
            kind: SemanticChangeKind::Removed,
            value: Some(value),
            before: None,
            after: None,
            risk: None,
        }
    }

    fn changed(path: impl Into<String>, before: Value, after: Value) -> Self {
        Self {
            path: path.into(),
            kind: SemanticChangeKind::Changed,
            value: None,
            before: Some(before),
            after: Some(after),
            risk: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SemanticChangeKind {
    Added,
    Removed,
    Changed,
}

fn find_manifest_file(path: &Path) -> Option<PathBuf> {
    ["manifest.yaml", "manifest.yml", "manifest.json"]
        .iter()
        .map(|file| path.join(file))
        .find(|path| path.exists())
}

fn parse_manifest_value(source: &Path, raw: &str) -> Result<Value> {
    if source.extension().and_then(|ext| ext.to_str()) == Some("json") {
        Ok(serde_json::from_str(raw)?)
    } else {
        Ok(serde_yaml::from_str::<Value>(raw)?)
    }
}

fn parse_skill_frontmatter(path: &Path) -> Result<Option<Value>> {
    let raw = fs::read_to_string(path)?;
    if !raw.starts_with("---\n") && !raw.starts_with("---\r\n") {
        return Ok(None);
    }
    let normalized = raw.replace("\r\n", "\n");
    let Some(end) = normalized[4..].find("\n---") else {
        return Ok(None);
    };
    let yaml = &normalized[4..4 + end];
    Ok(Some(serde_yaml::from_str::<Value>(yaml)?))
}

fn merge_frontmatter_defaults(
    warnings: &mut Vec<ManifestIssue>,
    manifest: &mut Value,
    frontmatter: &Value,
) {
    let (Some(manifest_obj), Some(frontmatter_obj)) =
        (manifest.as_object_mut(), frontmatter.as_object())
    else {
        return;
    };

    for (key, value) in frontmatter_obj {
        match manifest_obj.get(key) {
            Some(existing) if existing != value => warnings.push(ManifestIssue::new(
                key,
                "manifest file and SKILL.md frontmatter differ; manifest file wins",
            )),
            Some(_) => {}
            None => {
                manifest_obj.insert(key.clone(), value.clone());
            }
        }
    }
}

fn normalize_manifest_value(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    // Align with the agentskills.io open standard (and Claude Code's SKILL.md):
    // only `name` and `description` are required at minimum. Everything else
    // gets a sensible default so we accept any conformant skill folder.
    obj.entry("schemaVersion".to_owned())
        .or_insert_with(|| Value::Number(1.into()));
    obj.entry("type".to_owned())
        .or_insert_with(|| Value::String("skill".to_owned()));
    obj.entry("permissions".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()));
    obj.entry("targets".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()));
    obj.entry("tags".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()));
    obj.entry("version".to_owned())
        .or_insert_with(|| Value::String("0.0.0".to_owned()));
    // If `id` is missing, derive one from `name` so downstream code that keys
    // on id (subscriptions, lockfile, dedup) still works. The scan layer can
    // overwrite this with the directory name when more accurate.
    if !obj.contains_key("id") {
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            obj.insert(
                "id".to_owned(),
                Value::String(slugify_for_id(name)),
            );
        }
    }
}

fn slugify_for_id(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() < 2 {
        out = format!("skill-{out}").trim_end_matches('-').to_owned();
    }
    if out.len() > 80 {
        out.truncate(80);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

fn validate_manifest(manifest: &SkillManifest) -> Vec<ManifestIssue> {
    let mut errors = Vec::new();
    // Per https://agentskills.io specification, only `name` and `description`
    // are required at minimum. Treat everything else as warnings (they degrade
    // capability — the skill may not install everywhere — but don't make it
    // invalid to display in the workspace).
    if manifest.name.trim().is_empty() {
        errors.push(ManifestIssue::new("name", "is required"));
    }
    if manifest.description.trim().is_empty() {
        errors.push(ManifestIssue::new("description", "is required"));
    }
    errors
}

fn warnings_for_manifest(manifest: &SkillManifest) -> Vec<ManifestIssue> {
    let mut warnings = Vec::new();
    if manifest.asset_type != "skill" {
        warnings.push(ManifestIssue::new(
            "type",
            format!("expected `skill`, got `{}`", manifest.asset_type),
        ));
    }
    if !valid_id(&manifest.id) {
        warnings.push(ManifestIssue::new(
            "id",
            "should be 2-80 chars of lowercase letters, numbers, dots, underscores, or dashes",
        ));
    }
    if !manifest.version.is_empty() && Version::parse(&manifest.version).is_err() {
        warnings.push(ManifestIssue::new(
            "version",
            "should be SemVer without a leading v",
        ));
    }
    if manifest.targets.is_empty() {
        warnings.push(ManifestIssue::new(
            "targets",
            "no runtime targets declared; skill won't auto-install anywhere",
        ));
    }
    if manifest.name.trim().len() < 2 {
        warnings.push(ManifestIssue::new("name", "shorter than 2 characters"));
    }
    if manifest.description.trim().len() < 10 {
        warnings.push(ManifestIssue::new(
            "description",
            "shorter than 10 characters; may be hard to discover",
        ));
    }
    for permission in &manifest.permissions {
        if !KNOWN_PERMISSIONS.contains(&permission.as_str()) {
            warnings.push(ManifestIssue::new(
                "permissions",
                format!("unknown permission `{permission}`"),
            ));
        }
    }
    for target in &manifest.targets {
        if !KNOWN_TARGETS.contains(&target.as_str()) {
            warnings.push(ManifestIssue::new(
                "targets",
                format!("unknown runtime target `{target}`"),
            ));
        }
    }
    warnings
}

fn valid_id(id: &str) -> bool {
    let len = id.len();
    if !(2..=80).contains(&len) {
        return false;
    }
    let first_last_valid = id
        .chars()
        .next()
        .zip(id.chars().last())
        .map(|(first, last)| first.is_ascii_alphanumeric() && last.is_ascii_alphanumeric())
        .unwrap_or(false);
    first_last_valid
        && id.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '_' | '-')
        })
}

fn diff_string_set(path: &str, from: &[String], to: &[String], changes: &mut Vec<SemanticChange>) {
    let from = from.iter().collect::<BTreeSet<_>>();
    let to = to.iter().collect::<BTreeSet<_>>();
    for value in to.difference(&from) {
        changes.push(SemanticChange::added(
            format!("{path}.{}", value),
            Value::String((*value).clone()),
        ));
    }
    for value in from.difference(&to) {
        changes.push(SemanticChange::removed(
            format!("{path}.{}", value),
            Value::String((*value).clone()),
        ));
    }
}

const KNOWN_TARGETS: &[&str] = &["claude-code", "cursor", "codex"];
const KNOWN_PERMISSIONS: &[&str] = &[
    "filesystem.read",
    "filesystem.write",
    "shell.execute.limited",
    "shell.execute",
    "network.provider",
    "network.external",
    "secrets.read",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_yaml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 1.2.3
targets:
  - claude-code
permissions:
  - filesystem.read
"#,
        )
        .unwrap();
        fs::write(dir.path().join("SKILL.md"), "# Code Reviewer\n").unwrap();

        let parsed = parse_skill_dir(dir.path()).unwrap();
        assert!(parsed.ok(), "{parsed:?}");
        assert_eq!(parsed.manifest.unwrap().id, "code-reviewer");
    }

    #[test]
    fn frontmatter_can_define_manifest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            r#"---
id: pr-summarizer
type: skill
name: PR Summarizer
description: Summarizes pull requests for reviewers.
version: 0.1.0
targets:
  - cursor
---
# PR Summarizer
"#,
        )
        .unwrap();

        let parsed = parse_skill_dir(dir.path()).unwrap();
        assert!(parsed.ok(), "{parsed:?}");
        assert_eq!(parsed.manifest.unwrap().targets, vec!["cursor"]);
    }

    #[test]
    fn semantic_diff_highlights_permission_risk() {
        let mut from = minimal_manifest("1.0.0");
        let mut to = minimal_manifest("1.1.0");
        to.permissions.push("shell.execute".to_owned());

        let changes = semantic_diff(&from, &to);
        assert!(changes.iter().any(|change| {
            change.path == "permissions.shell.execute" && change.risk == Some(RiskLevel::High)
        }));
        from.permissions.push("filesystem.read".to_owned());
        assert!(!semantic_diff(&from, &to).is_empty());
    }

    fn minimal_manifest(version: &str) -> SkillManifest {
        SkillManifest {
            schema_version: 1,
            id: "code-reviewer".to_owned(),
            asset_type: "skill".to_owned(),
            name: "Code Reviewer".to_owned(),
            description: "Reviews changes for correctness.".to_owned(),
            version: version.to_owned(),
            license: None,
            homepage: None,
            repository: None,
            authors: Vec::new(),
            targets: vec!["claude-code".to_owned()],
            permissions: Vec::new(),
            tags: Vec::new(),
            runtime: BTreeMap::new(),
            compatibility: BTreeMap::new(),
            dependencies: BTreeMap::new(),
            files: None,
            risk: None,
            extra: BTreeMap::new(),
        }
    }
}
