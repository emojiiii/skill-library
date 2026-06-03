use sha2::{Digest, Sha256};
use skill_library_core::WorkspaceRef;
use skill_library_provider::{FileEntry, FileKind, ProviderError, Result};
use std::collections::{BTreeSet, VecDeque};
use std::path::Path;

use crate::index::WebDavIndex;
use crate::paths::{join_repo_path, normalize_repo_path_lossy};
use crate::WebDavProvider;

impl WebDavProvider {
    pub(crate) async fn list_collection_files(&self, root_path: &str) -> Result<Vec<FileEntry>> {
        let mut queue = VecDeque::from([String::new()]);
        let mut seen_dirs = BTreeSet::new();
        let mut seen_entries = BTreeSet::new();
        let mut files = Vec::new();

        while let Some(relative_dir) = queue.pop_front() {
            if !seen_dirs.insert(relative_dir.clone()) {
                continue;
            }
            let current = join_repo_path(root_path, &relative_dir);
            let entries = self.propfind_collection(&current, "1").await?;
            for entry in entries {
                let mut relative = normalize_repo_path_lossy(&entry.relative_path);
                if !relative_dir.is_empty()
                    && !relative.is_empty()
                    && !relative.starts_with(&format!("{relative_dir}/"))
                {
                    relative = join_repo_path(&relative_dir, &relative);
                }
                if relative.is_empty() || !seen_entries.insert(relative.clone()) {
                    continue;
                }
                let kind = if entry.is_collection {
                    FileKind::Directory
                } else {
                    FileKind::File
                };
                files.push(FileEntry {
                    path: relative.clone(),
                    kind,
                    sha: entry.stable_id(),
                    size: entry.content_length,
                });
                if entry.is_collection {
                    queue.push_back(relative);
                }
            }
        }

        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    pub(crate) async fn download_collection_into(
        &self,
        root_path: &str,
        output_dir: &Path,
        hasher: &mut Sha256,
        downloaded: &mut u64,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<()> {
        let files = self.list_collection_files(root_path).await?;
        for entry in files {
            let target = output_dir.join(&entry.path);
            if matches!(entry.kind, FileKind::Directory) {
                std::fs::create_dir_all(&target)
                    .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
                continue;
            }
            let remote_path = join_repo_path(root_path, &entry.path);
            let (_, bytes) = self.get_bytes(&remote_path).await?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
            }
            std::fs::write(&target, &bytes)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
            hasher.update(entry.path.as_bytes());
            hasher.update(&bytes);
            *downloaded += bytes.len() as u64;
            on_progress(*downloaded, None);
        }
        Ok(())
    }

    pub(crate) async fn download_indexed_snapshot(
        &self,
        reference: &WorkspaceRef,
        index: &WebDavIndex,
        version: Option<&str>,
        extracted_root: &Path,
        hasher: &mut Sha256,
        downloaded: &mut u64,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<()> {
        let workspace_root = Self::workspace_path(reference);
        let mut matched = 0_usize;
        for skill in &index.skills {
            let Some(skill_dir) = skill.dir_for_ref(version) else {
                continue;
            };
            matched += 1;
            let source_dir = join_repo_path(&workspace_root, &skill_dir);
            let target_dir = extracted_root.join(skill.display_path());
            std::fs::create_dir_all(&target_dir)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
            self.download_collection_into(
                &source_dir,
                &target_dir,
                hasher,
                downloaded,
                on_progress,
            )
            .await?;
        }
        if matched == 0 {
            let version = version.unwrap_or("latest");
            return Err(ProviderError::NotFound {
                resource: format!("WebDAV version '{version}'"),
                reference: Some(reference.full_name()),
            });
        }
        Ok(())
    }
}
