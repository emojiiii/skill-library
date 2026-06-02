use crate::GitHubProvider;
use skill_library_core::WorkspaceRef;
use skill_library_manifest::{parse_skill_dir, SkillAsset};
use skill_library_provider::{
    FileKind, GitRef, PageOpts, Provider, ProviderError, Result, Workspace,
};
use std::collections::BTreeMap;

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
    // No README probe — it's never rendered in the UI and only generated 404s.
    let versions = list_skill_versions(provider, reference).await?;
    Ok(WorkspaceDetailScan {
        workspace,
        skills,
        readme: None,
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
    let requested = normalize_skill_dir(skill_dir);

    // The caller may pass either a literal directory path (workspace browsing)
    // or just a skill id/name (the discovery/registry path, where skills often
    // live under a nested dir like `skills/<id>`). Try the literal path first;
    // if nothing is there, scan the repo once and resolve by SKILL.md metadata or path or the
    // directory's basename. The scan already parsed the manifest, so we reuse
    // that asset instead of re-fetching it (saves a round-trip per click).
    let (asset, resolved_dir) = match read_skill_asset(provider, reference, &at, requested).await {
        Ok(asset) => (asset, requested.to_owned()),
        Err(ProviderError::NotFound { .. }) => {
            let (dir, asset) = resolve_skill_asset(provider, reference, &at, requested).await?;
            (asset, dir)
        }
        Err(err) => return Err(err),
    };

    // skill_markdown / versions are independent — fetch concurrently. We do NOT
    // probe for a README: it's never rendered in the UI, and blind-probing
    // README.md/.mdx/readme.md just produced a burst of 404s per skill.
    let (skill_markdown, versions) = futures::try_join!(
        read_skill_markdown(provider, reference, &at, &resolved_dir),
        list_skill_versions(provider, reference),
    )?;

    Ok(SkillDetailScan {
        workspace,
        asset,
        readme: None,
        skill_markdown,
        versions,
        ref_name: ref_name.map(str::to_owned),
    })
}

/// Resolve a skill when the caller passed an id/name rather than the actual
/// in-repo path. Scans the repo's skills once and returns the matched asset
/// together with its normalized directory, so the caller can reuse the manifest
/// the scan already parsed. Matches by SKILL.md metadata or path or the directory's basename.
async fn resolve_skill_asset(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
    requested: &str,
) -> Result<(String, SkillAsset)> {
    let needle = requested.trim_matches('/');
    let skills = scan_skill_assets_at(provider, reference, at).await?;
    let dirs: Vec<(String, String)> = skills
        .iter()
        .map(|asset| {
            (
                normalize_asset_path(&asset.path.to_string_lossy()),
                asset.manifest.id.clone(),
            )
        })
        .collect();
    let Some(resolved) = match_skill_dir(&dirs, needle) else {
        return Err(ProviderError::NotFound {
            resource: format!("skill '{needle}' in {}", reference.full_name()),
            reference: None,
        });
    };
    let asset = skills
        .into_iter()
        .find(|asset| normalize_asset_path(&asset.path.to_string_lossy()) == resolved)
        .ok_or_else(|| ProviderError::NotFound {
            resource: format!("skill '{needle}' in {}", reference.full_name()),
            reference: None,
        })?;
    Ok((resolved, asset))
}

/// Normalize an in-repo skill path: forward slashes, no `./` prefix, no
/// surrounding slashes. (`.` — the repo root — becomes empty.)
fn normalize_asset_path(path: &str) -> String {
    let cleaned = path
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_owned();
    if cleaned == "." {
        String::new()
    } else {
        cleaned
    }
}

/// Given `(normalized_dir, manifest_id)` pairs, find the directory whose
/// SKILL.md metadata or path, full path, or final path component equals `needle`. Match
/// priority: exact path > SKILL.md metadata or path > basename, so a literal path always
/// wins over an id collision in a nested layout.
fn match_skill_dir(dirs: &[(String, String)], needle: &str) -> Option<String> {
    let needle = needle.trim_matches('/');
    if let Some((dir, _)) = dirs.iter().find(|(dir, _)| dir == needle) {
        return Some(dir.clone());
    }
    if let Some((dir, _)) = dirs.iter().find(|(_, id)| id == needle) {
        return Some(dir.clone());
    }
    dirs.iter()
        .find(|(dir, _)| dir.rsplit('/').next().unwrap_or(dir.as_str()) == needle)
        .map(|(dir, _)| dir.clone())
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

    // Directories that cannot contain skill manifests — skip them to reduce noise
    // and speed up scanning on large repos.
    const SKIP_PREFIXES: &[&str] = &[
        "node_modules/",
        ".git/",
        "target/",
        "dist/",
        "build/",
        ".next/",
        "__pycache__/",
        ".venv/",
        "vendor/",
        ".cargo/",
        ".gradle/",
        ".turbo/",
        ".output/",
        ".nuxt/",
    ];

    // Step 1 — single tree pass: identify every skill directory and the best
    // manifest entry inside it (yaml > yml > json > SKILL.md).
    let mut candidates: BTreeMap<String, ManifestKind> = BTreeMap::new();
    for entry in entries {
        if !matches!(entry.kind, FileKind::File) {
            continue;
        }
        // Skip files inside well-known non-skill directories
        if SKIP_PREFIXES.iter().any(|prefix| {
            entry.path.starts_with(prefix) || entry.path.contains(&format!("/{prefix}"))
        }) {
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
                target: "skill-library-github",
                skill_dir = %skill_dir,
                kind = ?kind,
                "manifest missing or binary, skipping"
            );
            continue;
        };

        let result = match kind {
            ManifestKind::Json => {
                serde_json::from_str::<skill_library_manifest::SkillManifest>(&text)
                    .map(|manifest| SkillAsset {
                        path: skill_path_buf(&skill_dir),
                        manifest,
                        warnings: Vec::new(),
                    })
                    .map_err(|err| err.to_string())
            }
            ManifestKind::Yaml | ManifestKind::Yml => {
                serde_yaml::from_str::<skill_library_manifest::SkillManifest>(&text)
                    .map(|manifest| SkillAsset {
                        path: skill_path_buf(&skill_dir),
                        manifest,
                        warnings: Vec::new(),
                    })
                    .map_err(|err| err.to_string())
            }
            ManifestKind::SkillMd => {
                parse_skill_md_text(&text, &skill_dir).map_err(|err| err.to_string())
            }
        };

        match result {
            Ok(asset) => skills.push(asset),
            Err(err) => {
                tracing::warn!(
                    target: "skill-library-github",
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

/// Streaming variant of `scan_skill_assets_at`. Calls `on_batch` after each
/// batch of manifests is fetched and parsed, allowing the caller to emit
/// incremental progress (e.g. Tauri events) before the full scan completes.
pub async fn scan_skill_assets_streaming(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
    mut on_batch: impl FnMut(&[SkillAsset]),
) -> Result<Vec<SkillAsset>> {
    let entries = provider.list_files(reference, at).await?;

    const SKIP_PREFIXES: &[&str] = &[
        "node_modules/",
        ".git/",
        "target/",
        "dist/",
        "build/",
        ".next/",
        "__pycache__/",
        ".venv/",
        "vendor/",
        ".cargo/",
        ".gradle/",
        ".turbo/",
        ".output/",
        ".nuxt/",
    ];

    let mut candidates: BTreeMap<String, ManifestKind> = BTreeMap::new();
    for entry in entries {
        if !matches!(entry.kind, FileKind::File) {
            continue;
        }
        if SKIP_PREFIXES.iter().any(|prefix| {
            entry.path.starts_with(prefix) || entry.path.contains(&format!("/{prefix}"))
        }) {
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

    // Process in batches, emitting each batch's results incrementally
    let mut all_skills: Vec<SkillAsset> = Vec::new();
    let dir_chunks: Vec<&[(String, ManifestKind)]> = dirs.chunks(BATCH).collect();
    let path_chunks: Vec<&[String]> = paths.chunks(BATCH).collect();

    for (dir_chunk, path_chunk) in dir_chunks.into_iter().zip(path_chunks.into_iter()) {
        let texts = provider
            .batch_fetch_text_files(reference, &ref_name, path_chunk)
            .await?;

        let mut batch_skills: Vec<SkillAsset> = Vec::new();
        for ((skill_dir, kind), text_opt) in dir_chunk.iter().zip(texts.into_iter()) {
            let Some(text) = text_opt else {
                continue;
            };
            let result = match kind {
                ManifestKind::Json => {
                    serde_json::from_str::<skill_library_manifest::SkillManifest>(&text)
                        .map(|manifest| SkillAsset {
                            path: skill_path_buf(skill_dir),
                            manifest,
                            warnings: Vec::new(),
                        })
                        .map_err(|err| err.to_string())
                }
                ManifestKind::Yaml | ManifestKind::Yml => {
                    serde_yaml::from_str::<skill_library_manifest::SkillManifest>(&text)
                        .map(|manifest| SkillAsset {
                            path: skill_path_buf(skill_dir),
                            manifest,
                            warnings: Vec::new(),
                        })
                        .map_err(|err| err.to_string())
                }
                ManifestKind::SkillMd => {
                    parse_skill_md_text(&text, skill_dir).map_err(|err| err.to_string())
                }
            };
            match result {
                Ok(asset) => batch_skills.push(asset),
                Err(err) => {
                    tracing::warn!(
                        target: "skill-library-github",
                        skill_dir = %skill_dir,
                        error = %err,
                        "skipping skill: failed to parse manifest"
                    );
                }
            }
        }

        if !batch_skills.is_empty() {
            on_batch(&batch_skills);
            all_skills.extend(batch_skills);
        }
    }

    all_skills.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    all_skills.dedup_by(|a, b| a.manifest.id == b.manifest.id);
    Ok(all_skills)
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
    // A skill ships its metadata as frontmatter inside SKILL.md — there is no
    // separate manifest.{yaml,yml,json} file. Read SKILL.md directly instead of
    // blind-probing manifest names (the old behavior 404'd 3 times per skill
    // before reaching SKILL.md). NotFound propagates to the caller, which falls
    // back to a full repo scan to resolve skills addressed by id rather than path.
    let path = if skill_dir.is_empty() {
        "SKILL.md".to_owned()
    } else {
        format!("{skill_dir}/SKILL.md")
    };
    let blob = provider.read_file(reference, at, &path).await?;
    let text = String::from_utf8(blob.bytes)
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    parse_skill_md_text(&text, skill_dir)
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

async fn read_first_markdown_owned(
    provider: &GitHubProvider,
    reference: &WorkspaceRef,
    at: &GitRef,
    candidates: Vec<String>,
) -> Result<Option<MarkdownDocument>> {
    // Probe candidate filenames concurrently, returning the first (by original
    // index) that resolves. Currently only called with a single SKILL.md path,
    // but kept general in case more candidates are needed.
    let results = futures::future::join_all(
        candidates
            .iter()
            .map(|path| async move { provider.read_file(reference, at, path).await }),
    )
    .await;

    for (path, result) in candidates.into_iter().zip(results.into_iter()) {
        match result {
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

    #[test]
    fn normalize_asset_path_strips_prefixes_and_root() {
        assert_eq!(
            normalize_asset_path("skills/find-skills"),
            "skills/find-skills"
        );
        assert_eq!(
            normalize_asset_path("./skills/find-skills"),
            "skills/find-skills"
        );
        assert_eq!(
            normalize_asset_path("/skills/find-skills/"),
            "skills/find-skills"
        );
        assert_eq!(
            normalize_asset_path("skills\\find-skills"),
            "skills/find-skills"
        );
        assert_eq!(normalize_asset_path("."), "");
    }

    /// The discovery/registry path passes a bare skill id (e.g. "find-skills")
    /// while the real repo nests it under "skills/find-skills". Matching by the
    /// directory's basename must resolve it.
    #[test]
    fn match_skill_dir_resolves_bare_id_to_nested_dir() {
        let dirs = vec![
            ("skills/find-skills".to_owned(), "find-skills".to_owned()),
            (
                "skills/frontend-design".to_owned(),
                "frontend-design".to_owned(),
            ),
        ];
        assert_eq!(
            match_skill_dir(&dirs, "find-skills"),
            Some("skills/find-skills".to_owned())
        );
    }

    /// Matching by SKILL.md metadata or path works even when it differs from the directory name.
    #[test]
    fn match_skill_dir_matches_manifest_id() {
        let dirs = vec![("skills/dir-name".to_owned(), "the-real-id".to_owned())];
        assert_eq!(
            match_skill_dir(&dirs, "the-real-id"),
            Some("skills/dir-name".to_owned())
        );
    }

    /// An exact path always wins over a basename collision in a nested layout.
    #[test]
    fn match_skill_dir_prefers_exact_path_over_basename() {
        let dirs = vec![
            ("a/shared".to_owned(), "a-shared".to_owned()),
            ("shared".to_owned(), "root-shared".to_owned()),
        ];
        assert_eq!(
            match_skill_dir(&dirs, "shared"),
            Some("shared".to_owned()),
            "literal top-level path must win over a nested basename match"
        );
    }

    #[test]
    fn match_skill_dir_returns_none_when_absent() {
        let dirs = vec![("skills/find-skills".to_owned(), "find-skills".to_owned())];
        assert_eq!(match_skill_dir(&dirs, "does-not-exist"), None);
    }
}
