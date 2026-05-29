use crate::GitHubProvider;
use std::collections::BTreeMap;
use teamai_core::WorkspaceRef;
use teamai_manifest::{parse_skill_dir, SkillAsset};
use teamai_provider::{FileKind, GitRef, PageOpts, Provider, ProviderError, Result, Workspace};

pub async fn scan_workspace_skills(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
) -> Result<WorkspaceSkillScan> {
    let workspace = provider.get_workspace(reference).await?;
    let branch = GitRef::Branch(workspace.default_branch.clone());
    let skills = scan_skill_assets_at(provider, reference, &branch).await?;
    Ok(WorkspaceSkillScan { workspace, skills })
}

pub async fn scan_workspace_detail(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
) -> Result<WorkspaceDetailScan> {
    let workspace = provider.get_workspace(reference).await?;
    let branch = GitRef::Branch(workspace.default_branch.clone());
    let skills = scan_skill_assets_at(provider, reference, &branch).await?;
    let readme = read_workspace_readme(provider, reference, &branch).await?;
    let versions = list_skill_versions(provider, reference).await?;
    Ok(WorkspaceDetailScan {
        workspace,
        skills,
        readme,
        versions,
    })
}

pub async fn read_skill_detail(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    skill_dir: &str,
    ref_name: Option<&str>,
) -> Result<SkillDetailScan> {
    let workspace = provider.get_workspace(reference).await?;
    let at = git_ref_from_name(ref_name, &workspace.default_branch);
    let asset = read_skill_asset(provider, reference, &at, normalize_skill_dir(skill_dir)).await?;
    let readme =
        read_skill_readme(provider, reference, &at, normalize_skill_dir(skill_dir)).await?;
    let skill_markdown =
        read_skill_markdown(provider, reference, &at, normalize_skill_dir(skill_dir)).await?;
    let versions = list_skill_versions(provider, reference).await?;
    Ok(SkillDetailScan {
        workspace,
        asset,
        readme,
        skill_markdown,
        versions,
        ref_name: ref_name.map(str::to_owned),
    })
}

/// Possible manifest entry points inside a skill directory, in priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestKind {
    Yaml,
    Yml,
    Json,
    SkillMd,
}

impl ManifestKind {
    fn from_filename(name: &str) -> Option<Self> {
        match name {
            "manifest.yaml" => Some(Self::Yaml),
            "manifest.yml" => Some(Self::Yml),
            "manifest.json" => Some(Self::Json),
            "SKILL.md" => Some(Self::SkillMd),
            _ => None,
        }
    }

    /// Lower number = higher priority (yaml > yml > json > SKILL.md).
    fn priority(self) -> u8 {
        match self {
            Self::Yaml => 0,
            Self::Yml => 1,
            Self::Json => 2,
            Self::SkillMd => 3,
        }
    }

    fn filename(self) -> &'static str {
        match self {
            Self::Yaml => "manifest.yaml",
            Self::Yml => "manifest.yml",
            Self::Json => "manifest.json",
            Self::SkillMd => "SKILL.md",
        }
    }
}

pub async fn scan_skill_assets_at(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
) -> Result<Vec<SkillAsset>> {
    let entries = provider.list_files(reference, at).await?;

    // Step 1 — single tree pass: identify every skill directory and the best
    // manifest entry inside it (yaml > yml > json > SKILL.md).
    let mut candidates: BTreeMap<String, ManifestKind> = BTreeMap::new();
    for entry in entries {
        if !matches!(entry.kind, FileKind::File) {
            continue;
        }
        let (skill_dir, filename) = match entry.path.rsplit_once('/') {
            Some((dir, name)) => (dir.to_owned(), name),
            None => (String::new(), entry.path.as_str()),
        };
        let Some(kind) = ManifestKind::from_filename(filename) else {
            continue;
        };
        candidates
            .entry(skill_dir)
            .and_modify(|existing| {
                if kind.priority() < existing.priority() {
                    *existing = kind;
                }
            })
            .or_insert(kind);
    }

    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    // Step 2 — batch-fetch every manifest in one or two GraphQL calls.
    // GitHub allows pretty deep selection sets, but cap each batch at 50 to
    // keep the alias list and response size reasonable.
    const BATCH: usize = 50;
    let ref_name = match at {
        GitRef::Branch(name) | GitRef::Tag(name) | GitRef::Sha(name) => name.clone(),
    };
    let dirs: Vec<(String, ManifestKind)> = candidates.into_iter().collect();

    let mut paths: Vec<String> = Vec::with_capacity(dirs.len());
    for (skill_dir, kind) in &dirs {
        paths.push(if skill_dir.is_empty() {
            kind.filename().to_owned()
        } else {
            format!("{skill_dir}/{}", kind.filename())
        });
    }

    let mut texts: Vec<Option<String>> = Vec::with_capacity(paths.len());
    for chunk in paths.chunks(BATCH) {
        let batch = provider
            .batch_fetch_text_files(reference, &ref_name, chunk)
            .await?;
        texts.extend(batch);
    }

    // Step 3 — parse each manifest. Skill-md fallback path needs disk because
    // parse_skill_dir reads SKILL.md frontmatter from a directory; we set up
    // a per-skill tempdir in that case. For yaml/yml/json we parse the string
    // directly — no disk required.
    let mut skills: Vec<SkillAsset> = Vec::new();
    for ((skill_dir, kind), text_opt) in dirs.into_iter().zip(texts.into_iter()) {
        let Some(text) = text_opt else {
            tracing::debug!(
                target: "teamai-github",
                skill_dir = %skill_dir,
                kind = ?kind,
                "manifest missing or binary, skipping"
            );
            continue;
        };

        let result = match kind {
            ManifestKind::Json => {
                serde_json::from_str::<teamai_manifest::SkillManifest>(&text)
                    .map(|manifest| SkillAsset {
                        path: skill_path_buf(&skill_dir),
                        manifest,
                        warnings: Vec::new(),
                    })
                    .map_err(|err| err.to_string())
            }
            ManifestKind::Yaml | ManifestKind::Yml => {
                serde_yaml::from_str::<teamai_manifest::SkillManifest>(&text)
                    .map(|manifest| SkillAsset {
                        path: skill_path_buf(&skill_dir),
                        manifest,
                        warnings: Vec::new(),
                    })
                    .map_err(|err| err.to_string())
            }
            ManifestKind::SkillMd => parse_skill_md_text(&text, &skill_dir).map_err(|err| err.to_string()),
        };

        match result {
            Ok(asset) => skills.push(asset),
            Err(err) => {
                tracing::warn!(
                    target: "teamai-github",
                    skill_dir = %skill_dir,
                    error = %err,
                    "skipping skill: failed to parse manifest"
                );
            }
        }
    }

    skills.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    skills.dedup_by(|a, b| a.manifest.id == b.manifest.id);
    Ok(skills)
}

fn skill_path_buf(skill_dir: &str) -> std::path::PathBuf {
    if skill_dir.is_empty() {
        std::path::PathBuf::from(".")
    } else {
        std::path::PathBuf::from(skill_dir)
    }
}

fn parse_skill_md_text(
    text: &str,
    skill_dir: &str,
) -> std::result::Result<SkillAsset, ProviderError> {
    let temp_dir =
        tempfile::tempdir().map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    std::fs::write(temp_dir.path().join("SKILL.md"), text.as_bytes())
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    let parsed = parse_skill_dir(temp_dir.path())
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    let manifest = parsed.manifest.ok_or_else(|| {
        ProviderError::InvalidResponse("failed to parse frontmatter manifest".to_owned())
    })?;
    Ok(SkillAsset {
        path: skill_path_buf(skill_dir),
        manifest,
        warnings: parsed.warnings,
    })
}

async fn read_skill_asset(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
    skill_dir: &str,
) -> Result<SkillAsset> {
    let manifest_candidates = [
        format!("{skill_dir}/manifest.yaml"),
        format!("{skill_dir}/manifest.yml"),
        format!("{skill_dir}/manifest.json"),
    ];

    for candidate in manifest_candidates {
        let path = if skill_dir.is_empty() {
            candidate.trim_start_matches('/').to_owned()
        } else {
            candidate.clone()
        };
        let raw = match provider.read_file(reference, at, &path).await {
            Ok(blob) => String::from_utf8(blob.bytes)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?,
            Err(ProviderError::NotFound { .. }) => continue,
            Err(err) => return Err(err),
        };

        let manifest: teamai_manifest::SkillManifest = if path.ends_with(".json") {
            serde_json::from_str::<teamai_manifest::SkillManifest>(&raw)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?
        } else {
            serde_yaml::from_str::<teamai_manifest::SkillManifest>(&raw)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?
        };

        return Ok(SkillAsset {
            path: if skill_dir.is_empty() {
                std::path::PathBuf::from(".")
            } else {
                std::path::PathBuf::from(skill_dir)
            },
            manifest,
            warnings: Vec::new(),
        });
    }

    let skill_path = if skill_dir.is_empty() {
        "SKILL.md".to_owned()
    } else {
        format!("{skill_dir}/SKILL.md")
    };
    let skill_md = provider.read_file(reference, at, &skill_path).await?;
    let temp_dir =
        tempfile::tempdir().map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    std::fs::write(temp_dir.path().join("SKILL.md"), skill_md.bytes)
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    let parsed = parse_skill_dir(temp_dir.path())
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    let manifest: teamai_manifest::SkillManifest = parsed.manifest.ok_or_else(|| {
        ProviderError::InvalidResponse("failed to parse frontmatter manifest".to_owned())
    })?;
    Ok(SkillAsset {
        path: if skill_dir.is_empty() {
            std::path::PathBuf::from(".")
        } else {
            std::path::PathBuf::from(skill_dir)
        },
        manifest,
        warnings: parsed.warnings,
    })
}

async fn read_workspace_readme(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
) -> Result<Option<MarkdownDocument>> {
    read_first_markdown(
        provider,
        reference,
        at,
        &["README.md", "README.mdx", "readme.md", "Readme.md"],
    )
    .await
}

async fn read_skill_readme(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
    skill_dir: &str,
) -> Result<Option<MarkdownDocument>> {
    let candidates = markdown_candidates(skill_dir, &["README.md", "README.mdx", "readme.md"]);
    read_first_markdown_owned(provider, reference, at, candidates).await
}

async fn read_skill_markdown(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
    skill_dir: &str,
) -> Result<Option<MarkdownDocument>> {
    let path = if skill_dir.is_empty() {
        "SKILL.md".to_owned()
    } else {
        format!("{skill_dir}/SKILL.md")
    };
    read_first_markdown_owned(provider, reference, at, vec![path]).await
}

async fn read_first_markdown(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
    candidates: &[&str],
) -> Result<Option<MarkdownDocument>> {
    read_first_markdown_owned(
        provider,
        reference,
        at,
        candidates
            .iter()
            .map(|candidate| candidate.to_string())
            .collect(),
    )
    .await
}

async fn read_first_markdown_owned(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
    candidates: Vec<String>,
) -> Result<Option<MarkdownDocument>> {
    for path in candidates {
        match provider.read_file(reference, at, &path).await {
            Ok(blob) => {
                let content = String::from_utf8(blob.bytes)
                    .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
                return Ok(Some(MarkdownDocument { path, content }));
            }
            Err(ProviderError::NotFound { .. }) => continue,
            Err(err) => return Err(err),
        }
    }
    Ok(None)
}

async fn list_skill_versions(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
) -> Result<Vec<SkillVersion>> {
    let tags = provider
        .list_tags(
            reference,
            PageOpts {
                cursor: None,
                per_page: Some(100),
            },
        )
        .await?;
    Ok(tags
        .items
        .into_iter()
        .map(|tag| SkillVersion {
            name: tag.name,
            sha: tag.sha,
        })
        .collect())
}

fn markdown_candidates(skill_dir: &str, filenames: &[&str]) -> Vec<String> {
    filenames
        .iter()
        .map(|filename| {
            if skill_dir.is_empty() {
                filename.to_string()
            } else {
                format!("{skill_dir}/{filename}")
            }
        })
        .collect()
}

fn git_ref_from_name(ref_name: Option<&str>, default_branch: &str) -> GitRef {
    let Some(ref_name) = ref_name.map(str::trim).filter(|value| !value.is_empty()) else {
        return GitRef::Branch(default_branch.to_owned());
    };
    if ref_name == default_branch {
        return GitRef::Branch(default_branch.to_owned());
    }
    if ref_name.len() == 40 && ref_name.chars().all(|c| c.is_ascii_hexdigit()) {
        return GitRef::Sha(ref_name.to_owned());
    }
    GitRef::Tag(ref_name.trim_start_matches("refs/tags/").to_owned())
}

fn normalize_skill_dir(skill_dir: &str) -> &str {
    let value = skill_dir.trim().trim_matches('/');
    if value == "." {
        ""
    } else {
        value
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceSkillScan {
    pub workspace: Workspace,
    pub skills: Vec<SkillAsset>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceDetailScan {
    pub workspace: Workspace,
    pub skills: Vec<SkillAsset>,
    pub readme: Option<MarkdownDocument>,
    pub versions: Vec<SkillVersion>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillDetailScan {
    pub workspace: Workspace,
    pub asset: SkillAsset,
    pub readme: Option<MarkdownDocument>,
    pub skill_markdown: Option<MarkdownDocument>,
    pub versions: Vec<SkillVersion>,
    pub ref_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MarkdownDocument {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillVersion {
    pub name: String,
    pub sha: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_ref_from_name_defaults_to_branch() {
        assert_eq!(
            git_ref_from_name(None, "main"),
            GitRef::Branch("main".to_owned())
        );
        assert_eq!(
            git_ref_from_name(Some("main"), "main"),
            GitRef::Branch("main".to_owned())
        );
    }

    #[test]
    fn git_ref_from_name_accepts_tag_and_sha() {
        assert_eq!(
            git_ref_from_name(Some("refs/tags/v1.2.0"), "main"),
            GitRef::Tag("v1.2.0".to_owned())
        );
        assert_eq!(
            git_ref_from_name(Some("0123456789abcdef0123456789abcdef01234567"), "main"),
            GitRef::Sha("0123456789abcdef0123456789abcdef01234567".to_owned())
        );
    }
}
