use async_trait::async_trait;
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ChangedFile, GitRef, GitRepositoryProvider, Page, PageOpts, PermissionLevel, RefComparison,
    Release, Result, Tag,
};

use crate::models::{CompareResponse, PermissionResponse, ReleaseResponse, TagResponse};
use crate::permissions::permission_from_name;
use crate::provider::GiteeProvider;
use crate::util::url_encode;

#[async_trait]
impl GitRepositoryProvider for GiteeProvider {
    async fn list_tags(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Tag>> {
        let (owner, repo) = Self::owner_repo(reference);
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let raw: Page<TagResponse> = self
            .get_page_json(&format!(
                "/repos/{owner}/{repo}/tags?per_page={per_page}&page={}",
                url_encode(&page)
            ))
            .await?;
        Ok(Page {
            items: raw.items.into_iter().map(Tag::from).collect(),
            next_cursor: raw.next_cursor,
        })
    }

    async fn list_releases(
        &self,
        reference: &WorkspaceRef,
        opts: PageOpts,
    ) -> Result<Page<Release>> {
        let (owner, repo) = Self::owner_repo(reference);
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let raw: Page<ReleaseResponse> = self
            .get_page_json(&format!(
                "/repos/{owner}/{repo}/releases?per_page={per_page}&page={}",
                url_encode(&page)
            ))
            .await?;
        Ok(Page {
            items: raw.items.into_iter().map(Release::from).collect(),
            next_cursor: raw.next_cursor,
        })
    }

    async fn compare_refs(
        &self,
        reference: &WorkspaceRef,
        base: &GitRef,
        head: &GitRef,
    ) -> Result<RefComparison> {
        let (owner, repo) = Self::owner_repo(reference);
        let raw: CompareResponse = self
            .get_json(&format!(
                "/repos/{owner}/{repo}/compare/{}...{}",
                url_encode(base.value()),
                url_encode(head.value())
            ))
            .await?;
        Ok(RefComparison {
            status: raw.status.unwrap_or_else(|| "ahead".to_owned()),
            ahead_by: raw.ahead_by.unwrap_or(raw.commits.len() as u32),
            behind_by: raw.behind_by.unwrap_or(0),
            files: raw.files.into_iter().map(ChangedFile::from).collect(),
        })
    }

    async fn check_permission(
        &self,
        reference: &WorkspaceRef,
        login: &str,
    ) -> Result<PermissionLevel> {
        let (owner, repo) = Self::owner_repo(reference);
        let raw: PermissionResponse = self
            .get_json(&format!(
                "/repos/{owner}/{repo}/collaborators/{}/permission",
                url_encode(login)
            ))
            .await?;
        Ok(permission_from_name(&raw.permission))
    }
}
