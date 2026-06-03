use crate::{sync_provider_error, RemoteWorkspaceSkills, Result, SyncError};
use async_trait::async_trait;
use skill_library_core::WorkspaceRef;
use skill_library_manifest::{parse_skill_dir, SkillAsset};
use skill_library_provider::{
    FileKind, GitRepositoryProvider, ProviderError, Result as ProviderResult, SkillSourceProvider,
    SourceRef,
};
use skill_library_provider_gitee::GiteeProvider;
use skill_library_provider_github::{scan, GitHubProvider};
use skill_library_provider_gitlab::GitLabProvider;
use skill_library_provider_webdav::{WebDavIndex, WebDavIndexSkill, WebDavProvider};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub use skill_library_provider_github::scan::{
    MarkdownDocument, SkillDetailScan, SkillVersion, WorkspaceDetailScan,
};

#[async_trait]
pub trait SkillCatalogProvider: Send + Sync {
    async fn scan_workspace_skills(
        &self,
        workspace: &WorkspaceRef,
    ) -> Result<RemoteWorkspaceSkills>;

    async fn scan_workspace_skills_streaming(
        &self,
        workspace: &WorkspaceRef,
        on_batch: &mut (dyn for<'a> FnMut(&'a [SkillAsset]) + Send),
    ) -> Result<RemoteWorkspaceSkills>;

    async fn scan_workspace_detail(&self, workspace: &WorkspaceRef) -> Result<WorkspaceDetailScan>;

    async fn read_skill_detail(
        &self,
        workspace: &WorkspaceRef,
        skill_path: &str,
        ref_name: Option<&str>,
    ) -> Result<SkillDetailScan>;
}

#[async_trait]
impl SkillCatalogProvider for GitHubProvider {
    async fn scan_workspace_skills(
        &self,
        workspace: &WorkspaceRef,
    ) -> Result<RemoteWorkspaceSkills> {
        let scan = scan::scan_workspace_skills(self, workspace)
            .await
            .map_err(sync_provider_error)?;
        Ok(RemoteWorkspaceSkills {
            workspace: scan.workspace,
            skills: scan.skills,
        })
    }

    async fn scan_workspace_skills_streaming(
        &self,
        workspace: &WorkspaceRef,
        on_batch: &mut (dyn for<'a> FnMut(&'a [SkillAsset]) + Send),
    ) -> Result<RemoteWorkspaceSkills> {
        let ws = skill_library_provider::Provider::get_workspace(self, workspace)
            .await
            .map_err(sync_provider_error)?;
        let branch = skill_library_provider::GitRef::Branch(ws.default_branch.clone());
        let skills = scan::scan_skill_assets_streaming(self, workspace, &branch, |batch| {
            on_batch(batch);
        })
        .await
        .map_err(sync_provider_error)?;
        Ok(RemoteWorkspaceSkills {
            workspace: ws,
            skills,
        })
    }

    async fn scan_workspace_detail(&self, workspace: &WorkspaceRef) -> Result<WorkspaceDetailScan> {
        scan::scan_workspace_detail(self, workspace)
            .await
            .map_err(sync_provider_error)
    }

    async fn read_skill_detail(
        &self,
        workspace: &WorkspaceRef,
        skill_path: &str,
        ref_name: Option<&str>,
    ) -> Result<SkillDetailScan> {
        scan::read_skill_detail(self, workspace, skill_path, ref_name)
            .await
            .map_err(sync_provider_error)
    }
}

#[async_trait]
impl SkillCatalogProvider for GitLabProvider {
    async fn scan_workspace_skills(
        &self,
        workspace: &WorkspaceRef,
    ) -> Result<RemoteWorkspaceSkills> {
        scan_workspace_skills_generic(self, workspace).await
    }

    async fn scan_workspace_skills_streaming(
        &self,
        workspace: &WorkspaceRef,
        on_batch: &mut (dyn for<'a> FnMut(&'a [SkillAsset]) + Send),
    ) -> Result<RemoteWorkspaceSkills> {
        scan_workspace_skills_streaming_generic(self, workspace, on_batch).await
    }

    async fn scan_workspace_detail(&self, workspace: &WorkspaceRef) -> Result<WorkspaceDetailScan> {
        let remote = scan_workspace_skills_generic(self, workspace).await?;
        let versions = list_skill_versions(self, workspace).await?;
        Ok(WorkspaceDetailScan {
            workspace: remote.workspace,
            skills: remote.skills,
            readme: None,
            versions,
        })
    }

    async fn read_skill_detail(
        &self,
        workspace: &WorkspaceRef,
        skill_path: &str,
        ref_name: Option<&str>,
    ) -> Result<SkillDetailScan> {
        read_skill_detail_generic(self, workspace, skill_path, ref_name).await
    }
}

#[async_trait]
impl SkillCatalogProvider for GiteeProvider {
    async fn scan_workspace_skills(
        &self,
        workspace: &WorkspaceRef,
    ) -> Result<RemoteWorkspaceSkills> {
        scan_workspace_skills_generic(self, workspace).await
    }

    async fn scan_workspace_skills_streaming(
        &self,
        workspace: &WorkspaceRef,
        on_batch: &mut (dyn for<'a> FnMut(&'a [SkillAsset]) + Send),
    ) -> Result<RemoteWorkspaceSkills> {
        scan_workspace_skills_streaming_generic(self, workspace, on_batch).await
    }

    async fn scan_workspace_detail(&self, workspace: &WorkspaceRef) -> Result<WorkspaceDetailScan> {
        let remote = scan_workspace_skills_generic(self, workspace).await?;
        let versions = list_skill_versions(self, workspace).await?;
        Ok(WorkspaceDetailScan {
            workspace: remote.workspace,
            skills: remote.skills,
            readme: None,
            versions,
        })
    }

    async fn read_skill_detail(
        &self,
        workspace: &WorkspaceRef,
        skill_path: &str,
        ref_name: Option<&str>,
    ) -> Result<SkillDetailScan> {
        read_skill_detail_generic(self, workspace, skill_path, ref_name).await
    }
}

#[async_trait]
impl SkillCatalogProvider for WebDavProvider {
    async fn scan_workspace_skills(
        &self,
        workspace: &WorkspaceRef,
    ) -> Result<RemoteWorkspaceSkills> {
        let remote = self
            .get_source(workspace)
            .await
            .map_err(sync_provider_error)?;
        let skills = match self
            .read_index(workspace)
            .await
            .map_err(sync_provider_error)?
        {
            Some(index) => {
                let mut no_op = |_: &[SkillAsset]| {};
                scan_webdav_indexed_skills(self, workspace, &index, &mut no_op).await?
            }
            None => scan_skill_assets_at(self, workspace, &SourceRef::Latest)
                .await
                .map_err(sync_provider_error)?,
        };
        Ok(RemoteWorkspaceSkills {
            workspace: remote,
            skills,
        })
    }

    async fn scan_workspace_skills_streaming(
        &self,
        workspace: &WorkspaceRef,
        on_batch: &mut (dyn for<'a> FnMut(&'a [SkillAsset]) + Send),
    ) -> Result<RemoteWorkspaceSkills> {
        let remote = self
            .get_source(workspace)
            .await
            .map_err(sync_provider_error)?;
        let skills = match self
            .read_index(workspace)
            .await
            .map_err(sync_provider_error)?
        {
            Some(index) => scan_webdav_indexed_skills(self, workspace, &index, on_batch).await?,
            None => scan_skill_assets_streaming_at(self, workspace, &SourceRef::Latest, on_batch)
                .await
                .map_err(sync_provider_error)?,
        };
        Ok(RemoteWorkspaceSkills {
            workspace: remote,
            skills,
        })
    }

    async fn scan_workspace_detail(&self, workspace: &WorkspaceRef) -> Result<WorkspaceDetailScan> {
        let remote = self
            .get_source(workspace)
            .await
            .map_err(sync_provider_error)?;
        let index = self
            .read_index(workspace)
            .await
            .map_err(sync_provider_error)?;
        let skills = match index.as_ref() {
            Some(index) => {
                let mut no_op = |_: &[SkillAsset]| {};
                scan_webdav_indexed_skills(self, workspace, index, &mut no_op).await?
            }
            None => scan_skill_assets_at(self, workspace, &SourceRef::Latest)
                .await
                .map_err(sync_provider_error)?,
        };
        Ok(WorkspaceDetailScan {
            workspace: remote,
            skills,
            readme: None,
            versions: index
                .as_ref()
                .map(webdav_workspace_versions)
                .unwrap_or_default(),
        })
    }

    async fn read_skill_detail(
        &self,
        workspace: &WorkspaceRef,
        skill_path: &str,
        ref_name: Option<&str>,
    ) -> Result<SkillDetailScan> {
        let Some(index) = self
            .read_index(workspace)
            .await
            .map_err(sync_provider_error)?
        else {
            return read_skill_detail_generic(self, workspace, skill_path, ref_name).await;
        };
        let Some(skill) = find_webdav_index_skill(&index, skill_path) else {
            return read_skill_detail_generic(self, workspace, skill_path, ref_name).await;
        };
        let remote = self
            .get_source(workspace)
            .await
            .map_err(sync_provider_error)?;
        let version = ref_name.map(str::trim).filter(|value| !value.is_empty());
        let actual_dir = skill.dir_for_ref(version).ok_or_else(|| {
            SyncError::NotFound(format!(
                "version '{}' for skill '{}'",
                version.unwrap_or("latest"),
                skill.id
            ))
        })?;
        let display_dir = skill.display_path();
        let asset = read_indexed_skill_asset(self, workspace, &actual_dir, &display_dir)
            .await
            .map_err(sync_provider_error)?;
        let skill_markdown =
            read_indexed_skill_markdown(self, workspace, &actual_dir, &display_dir)
                .await
                .map_err(sync_provider_error)?;
        Ok(SkillDetailScan {
            workspace: remote,
            asset,
            readme: None,
            skill_markdown,
            versions: webdav_skill_versions(skill),
            ref_name: version.map(str::to_owned),
        })
    }
}

async fn scan_workspace_skills_generic(
    source: &(dyn SkillSourceProvider + Sync),
    workspace: &WorkspaceRef,
) -> Result<RemoteWorkspaceSkills> {
    let remote = source
        .get_source(workspace)
        .await
        .map_err(sync_provider_error)?;
    let skills = scan_skill_assets_at(source, workspace, &SourceRef::Latest)
        .await
        .map_err(sync_provider_error)?;
    Ok(RemoteWorkspaceSkills {
        workspace: remote,
        skills,
    })
}

async fn scan_workspace_skills_streaming_generic(
    source: &(dyn SkillSourceProvider + Sync),
    workspace: &WorkspaceRef,
    on_batch: &mut (dyn for<'a> FnMut(&'a [SkillAsset]) + Send),
) -> Result<RemoteWorkspaceSkills> {
    let remote = source
        .get_source(workspace)
        .await
        .map_err(sync_provider_error)?;
    let skills = scan_skill_assets_streaming_at(source, workspace, &SourceRef::Latest, on_batch)
        .await
        .map_err(sync_provider_error)?;
    Ok(RemoteWorkspaceSkills {
        workspace: remote,
        skills,
    })
}

async fn read_skill_detail_generic(
    source: &(dyn SkillSourceProvider + Sync),
    workspace: &WorkspaceRef,
    skill_path: &str,
    ref_name: Option<&str>,
) -> Result<SkillDetailScan> {
    let remote = source
        .get_source(workspace)
        .await
        .map_err(sync_provider_error)?;
    let at = ref_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| SourceRef::Revision(value.to_owned()))
        .unwrap_or(SourceRef::Latest);
    let requested = normalize_skill_dir(skill_path);
    let (asset, resolved_dir) = match read_skill_asset(source, workspace, &at, requested).await {
        Ok(asset) => (asset, requested.to_owned()),
        Err(ProviderError::NotFound { .. }) => {
            let (dir, asset) = resolve_skill_asset(source, workspace, &at, requested)
                .await
                .map_err(sync_provider_error)?;
            (asset, dir)
        }
        Err(err) => return Err(sync_provider_error(err)),
    };
    let skill_markdown = read_skill_markdown(source, workspace, &at, &resolved_dir)
        .await
        .map_err(sync_provider_error)?;
    Ok(SkillDetailScan {
        workspace: remote,
        asset,
        readme: None,
        skill_markdown,
        versions: Vec::new(),
        ref_name: ref_name.map(str::to_owned),
    })
}

async fn scan_skill_assets_at(
    source: &(dyn SkillSourceProvider + Sync),
    reference: &WorkspaceRef,
    at: &SourceRef,
) -> ProviderResult<Vec<SkillAsset>> {
    let mut no_op = |_: &[SkillAsset]| {};
    scan_skill_assets_streaming_at(source, reference, at, &mut no_op).await
}

async fn scan_skill_assets_streaming_at(
    source: &(dyn SkillSourceProvider + Sync),
    reference: &WorkspaceRef,
    at: &SourceRef,
    on_batch: &mut (dyn for<'a> FnMut(&'a [SkillAsset]) + Send),
) -> ProviderResult<Vec<SkillAsset>> {
    let entries = source.list_files(reference, at).await?;
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

    let dirs: Vec<(String, ManifestKind)> = candidates.into_iter().collect();
    const BATCH: usize = 25;
    let mut all_skills = Vec::new();
    for chunk in dirs.chunks(BATCH) {
        let mut batch_skills = Vec::new();
        for (skill_dir, kind) in chunk {
            let path = manifest_path(skill_dir, *kind);
            let blob = match source.read_file(reference, at, &path).await {
                Ok(blob) => blob,
                Err(ProviderError::NotFound { .. }) => continue,
                Err(err) => return Err(err),
            };
            let Ok(text) = String::from_utf8(blob.bytes) else {
                continue;
            };
            match parse_manifest_text(&text, skill_dir, *kind) {
                Ok(asset) => batch_skills.push(asset),
                Err(err) => {
                    tracing::warn!(
                        target: "skill-library-remote-scan",
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

async fn resolve_skill_asset(
    source: &(dyn SkillSourceProvider + Sync),
    reference: &WorkspaceRef,
    at: &SourceRef,
    requested: &str,
) -> ProviderResult<(String, SkillAsset)> {
    let needle = requested.trim_matches('/');
    let skills = scan_skill_assets_at(source, reference, at).await?;
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

async fn read_skill_asset(
    source: &(dyn SkillSourceProvider + Sync),
    reference: &WorkspaceRef,
    at: &SourceRef,
    skill_dir: &str,
) -> ProviderResult<SkillAsset> {
    let path = if skill_dir.is_empty() {
        "SKILL.md".to_owned()
    } else {
        format!("{skill_dir}/SKILL.md")
    };
    let blob = source.read_file(reference, at, &path).await?;
    let text = String::from_utf8(blob.bytes)
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    parse_skill_md_text(&text, skill_dir)
}

async fn read_skill_markdown(
    source: &(dyn SkillSourceProvider + Sync),
    reference: &WorkspaceRef,
    at: &SourceRef,
    skill_dir: &str,
) -> ProviderResult<Option<MarkdownDocument>> {
    let path = if skill_dir.is_empty() {
        "SKILL.md".to_owned()
    } else {
        format!("{skill_dir}/SKILL.md")
    };
    match source.read_file(reference, at, &path).await {
        Ok(blob) => {
            let content = String::from_utf8(blob.bytes)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
            Ok(Some(MarkdownDocument { path, content }))
        }
        Err(ProviderError::NotFound { .. }) => Ok(None),
        Err(err) => Err(err),
    }
}

async fn list_skill_versions(
    git: &(dyn GitRepositoryProvider + Sync),
    reference: &WorkspaceRef,
) -> Result<Vec<SkillVersion>> {
    let tags = git
        .list_tags(
            reference,
            skill_library_provider::PageOpts {
                cursor: None,
                per_page: Some(100),
            },
        )
        .await
        .map_err(sync_provider_error)?;
    Ok(tags
        .items
        .into_iter()
        .map(|tag| SkillVersion {
            name: tag.name,
            sha: tag.sha,
        })
        .collect())
}

async fn scan_webdav_indexed_skills(
    source: &WebDavProvider,
    reference: &WorkspaceRef,
    index: &WebDavIndex,
    on_batch: &mut (dyn for<'a> FnMut(&'a [SkillAsset]) + Send),
) -> Result<Vec<SkillAsset>> {
    let mut skills = Vec::new();
    for skill in &index.skills {
        let Some(actual_dir) = skill.dir_for_ref(None) else {
            continue;
        };
        let display_dir = skill.display_path();
        match read_indexed_skill_asset(source, reference, &actual_dir, &display_dir).await {
            Ok(asset) => {
                on_batch(std::slice::from_ref(&asset));
                skills.push(asset);
            }
            Err(ProviderError::NotFound { .. }) => continue,
            Err(err) => return Err(sync_provider_error(err)),
        }
    }
    skills.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    skills.dedup_by(|a, b| a.manifest.id == b.manifest.id);
    Ok(skills)
}

async fn read_indexed_skill_asset(
    source: &(dyn SkillSourceProvider + Sync),
    reference: &WorkspaceRef,
    actual_dir: &str,
    display_dir: &str,
) -> ProviderResult<SkillAsset> {
    let mut last_not_found = None;
    for kind in [
        ManifestKind::SkillMd,
        ManifestKind::Yaml,
        ManifestKind::Yml,
        ManifestKind::Json,
    ] {
        let path = manifest_path(actual_dir, kind);
        let blob = match source.read_file(reference, &SourceRef::Latest, &path).await {
            Ok(blob) => blob,
            Err(ProviderError::NotFound {
                resource,
                reference,
            }) => {
                last_not_found = Some(ProviderError::NotFound {
                    resource,
                    reference,
                });
                continue;
            }
            Err(err) => return Err(err),
        };
        let text = String::from_utf8(blob.bytes)
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        return parse_manifest_text(&text, display_dir, kind)
            .map_err(ProviderError::InvalidResponse);
    }
    Err(last_not_found.unwrap_or_else(|| ProviderError::NotFound {
        resource: format!("skill manifest in {actual_dir}"),
        reference: Some(reference.full_name()),
    }))
}

async fn read_indexed_skill_markdown(
    source: &(dyn SkillSourceProvider + Sync),
    reference: &WorkspaceRef,
    actual_dir: &str,
    display_dir: &str,
) -> ProviderResult<Option<MarkdownDocument>> {
    let path = manifest_path(actual_dir, ManifestKind::SkillMd);
    match source.read_file(reference, &SourceRef::Latest, &path).await {
        Ok(blob) => {
            let content = String::from_utf8(blob.bytes)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
            Ok(Some(MarkdownDocument {
                path: manifest_path(display_dir, ManifestKind::SkillMd),
                content,
            }))
        }
        Err(ProviderError::NotFound { .. }) => Ok(None),
        Err(err) => Err(err),
    }
}

fn find_webdav_index_skill<'a>(
    index: &'a WebDavIndex,
    skill_path: &str,
) -> Option<&'a WebDavIndexSkill> {
    let needle = normalize_asset_path(skill_path);
    index.skills.iter().find(|skill| {
        let display = normalize_asset_path(&skill.display_path());
        let basename = display.rsplit('/').next().unwrap_or(display.as_str());
        skill.id == needle || display == needle || basename == needle
    })
}

fn webdav_skill_versions(skill: &WebDavIndexSkill) -> Vec<SkillVersion> {
    skill
        .versions
        .iter()
        .map(|(name, path)| SkillVersion {
            name: name.clone(),
            sha: skill
                .checksum
                .clone()
                .or_else(|| skill.dir_for_ref(Some(name)))
                .unwrap_or_else(|| path.clone()),
        })
        .collect()
}

fn webdav_workspace_versions(index: &WebDavIndex) -> Vec<SkillVersion> {
    let mut versions = BTreeMap::new();
    for skill in &index.skills {
        for version in webdav_skill_versions(skill) {
            versions.entry(version.name.clone()).or_insert(version);
        }
    }
    versions.into_values().collect()
}

fn manifest_path(skill_dir: &str, kind: ManifestKind) -> String {
    if skill_dir.is_empty() {
        kind.filename().to_owned()
    } else {
        format!("{skill_dir}/{}", kind.filename())
    }
}

fn parse_manifest_text(
    text: &str,
    skill_dir: &str,
    kind: ManifestKind,
) -> std::result::Result<SkillAsset, String> {
    match kind {
        ManifestKind::Json => serde_json::from_str::<skill_library_manifest::SkillManifest>(text)
            .map(|manifest| SkillAsset {
                path: skill_path_buf(skill_dir),
                manifest,
                warnings: Vec::new(),
            })
            .map_err(|err| err.to_string()),
        ManifestKind::Yaml | ManifestKind::Yml => {
            serde_yaml::from_str::<skill_library_manifest::SkillManifest>(text)
                .map(|manifest| SkillAsset {
                    path: skill_path_buf(skill_dir),
                    manifest,
                    warnings: Vec::new(),
                })
                .map_err(|err| err.to_string())
        }
        ManifestKind::SkillMd => {
            parse_skill_md_text(text, skill_dir).map_err(|err| err.to_string())
        }
    }
}

fn parse_skill_md_text(text: &str, skill_dir: &str) -> ProviderResult<SkillAsset> {
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

fn skill_path_buf(skill_dir: &str) -> PathBuf {
    if skill_dir.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(skill_dir)
    }
}

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

fn normalize_skill_dir(skill_dir: &str) -> &str {
    let value = skill_dir.trim().trim_matches('/');
    if value == "." {
        ""
    } else {
        value
    }
}
