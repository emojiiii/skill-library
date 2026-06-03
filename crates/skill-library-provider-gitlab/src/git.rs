use async_trait::async_trait;
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ChangedFile, GitRef, GitRepositoryProvider, Page, PageOpts, PermissionLevel, RefComparison,
    Release, Result, Tag,
};

use crate::models::{CompareResponse, MemberResponse, ReleaseResponse, TagResponse};
use crate::permissions::permission_from_access_level;
use crate::provider::GitLabProvider;
use crate::util::url_encode;

#[async_trait]
impl GitRepositoryProvider for GitLabProvider {
    async fn list_tags(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Tag>> {
        let project = Self::project_id(reference);
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let raw: Page<TagResponse> = self
            .get_page_json(&format!(
                "/projects/{project}/repository/tags?per_page={per_page}&page={}",
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
        let project = Self::project_id(reference);
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let raw: Page<ReleaseResponse> = self
            .get_page_json(&format!(
                "/projects/{project}/releases?per_page={per_page}&page={}",
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
        let project = Self::project_id(reference);
        let raw: CompareResponse = self
            .get_json(&format!(
                "/projects/{project}/repository/compare?from={}&to={}",
                url_encode(base.value()),
                url_encode(head.value())
            ))
            .await?;
        let identical = raw.compare_same_ref.unwrap_or(false) || raw.commits.is_empty();
        Ok(RefComparison {
            status: if identical { "identical" } else { "ahead" }.to_owned(),
            ahead_by: raw.commits.len() as u32,
            behind_by: 0,
            files: raw.diffs.into_iter().map(ChangedFile::from).collect(),
        })
    }

    async fn check_permission(
        &self,
        reference: &WorkspaceRef,
        login: &str,
    ) -> Result<PermissionLevel> {
        let project = Self::project_id(reference);
        let raw: Vec<MemberResponse> = self
            .get_json(&format!(
                "/projects/{project}/members/all?query={}&per_page=100",
                url_encode(login)
            ))
            .await?;
        Ok(raw
            .into_iter()
            .find(|member| member.username == login)
            .map(|member| permission_from_access_level(member.access_level))
            .unwrap_or(PermissionLevel::None))
    }
}
