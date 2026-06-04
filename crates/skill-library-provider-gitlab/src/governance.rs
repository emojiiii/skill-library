use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ChangedFile, Invitation, InvitationInput, IssueComment, Member, Page, PageOpts,
    PermissionLevel, ProviderError, PullRequest, PullRequestQueryState, PullRequestSummary,
    RepositoryEvent, Result,
};

use crate::models::{
    GitLabUserResponse, MemberAccessRequest, MemberUpdateRequest, MergeRequestChangesResponse,
    MergeRequestCloseRequest, MergeRequestNoteRequest, MergeRequestNoteResponse,
    MergeRequestResponse, ProjectEventResponse, ProjectInvitationRequest,
};
use crate::provider::GitLabProvider;
use crate::util::url_encode;

impl GitLabProvider {
    pub async fn list_merge_requests(
        &self,
        reference: &WorkspaceRef,
        state: PullRequestQueryState,
    ) -> Result<Vec<PullRequestSummary>> {
        let project = Self::project_id(reference);
        let state = match state {
            PullRequestQueryState::Open => "opened",
            PullRequestQueryState::Closed => "closed",
            PullRequestQueryState::All => "all",
        };
        let raw: Vec<MergeRequestResponse> = self
            .get_json(&format!(
                "/projects/{project}/merge_requests?state={state}&scope=all&order_by=updated_at&sort=desc&per_page=50"
            ))
            .await?;
        Ok(raw.into_iter().map(PullRequestSummary::from).collect())
    }

    pub async fn list_merge_request_files(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<Vec<ChangedFile>> {
        let project = Self::project_id(reference);
        let raw: MergeRequestChangesResponse = self
            .get_json(&format!(
                "/projects/{project}/merge_requests/{number}/changes"
            ))
            .await?;
        Ok(raw.changes.into_iter().map(ChangedFile::from).collect())
    }

    pub async fn list_repository_events(
        &self,
        reference: &WorkspaceRef,
    ) -> Result<Vec<RepositoryEvent>> {
        let project = Self::project_id(reference);
        let raw: Vec<ProjectEventResponse> = self
            .get_json(&format!("/projects/{project}/events?per_page=30"))
            .await?;
        Ok(raw.into_iter().map(RepositoryEvent::from).collect())
    }

    pub async fn list_project_members(
        &self,
        reference: &WorkspaceRef,
        opts: PageOpts,
    ) -> Result<Page<Member>> {
        let project = Self::project_id(reference);
        let per_page = opts.per_page.unwrap_or(100);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let raw: Page<crate::models::MemberResponse> = self
            .get_page_json(&format!(
                "/projects/{project}/members/all?per_page={per_page}&page={}",
                url_encode(&page)
            ))
            .await?;
        Ok(Page {
            items: raw.items.into_iter().map(Member::from).collect(),
            next_cursor: raw.next_cursor,
        })
    }

    pub async fn merge_merge_request(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<PullRequest> {
        let project = Self::project_id(reference);
        let raw: MergeRequestResponse = self
            .put_json(
                &format!("/projects/{project}/merge_requests/{number}/merge"),
                &serde_json::json!({ "squash": true }),
            )
            .await?;
        Ok(PullRequest {
            number: raw.iid,
            title: raw.title,
            html_url: raw.web_url,
            state: if raw.state == "merged" || raw.merged_at.is_some() {
                "closed".to_owned()
            } else {
                raw.state
            },
        })
    }

    pub async fn close_merge_request(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<PullRequestSummary> {
        let project = Self::project_id(reference);
        let raw: MergeRequestResponse = self
            .put_json(
                &format!("/projects/{project}/merge_requests/{number}"),
                &MergeRequestCloseRequest {
                    state_event: "close",
                },
            )
            .await?;
        Ok(raw.into())
    }

    pub async fn add_merge_request_comment(
        &self,
        reference: &WorkspaceRef,
        number: u64,
        body: &str,
    ) -> Result<IssueComment> {
        let project = Self::project_id(reference);
        let raw: MergeRequestNoteResponse = self
            .post_json(
                &format!("/projects/{project}/merge_requests/{number}/notes"),
                &MergeRequestNoteRequest { body },
            )
            .await?;
        Ok(raw.into())
    }

    pub async fn create_project_invitation(
        &self,
        reference: &WorkspaceRef,
        input: InvitationInput,
    ) -> Result<Invitation> {
        let project = Self::project_id(reference);
        let access_level = gitlab_access_level(&input.role);
        if input.login_or_email.contains('@') {
            self.post_status(
                &format!("/projects/{project}/invitations"),
                &ProjectInvitationRequest {
                    email: &input.login_or_email,
                    access_level,
                },
            )
            .await?;
            return Ok(Invitation {
                id: input.login_or_email.clone(),
                login_or_email: input.login_or_email,
                state: "pending".to_owned(),
            });
        }

        let user_id = self.resolve_user_id(&input.login_or_email).await?;
        let member: crate::models::MemberResponse = self
            .post_json(
                &format!("/projects/{project}/members"),
                &MemberAccessRequest {
                    user_id,
                    access_level,
                },
            )
            .await?;
        Ok(Invitation {
            id: user_id.to_string(),
            login_or_email: member.username,
            state: "active".to_owned(),
        })
    }

    pub async fn update_project_member_role(
        &self,
        reference: &WorkspaceRef,
        login: &str,
        role: PermissionLevel,
    ) -> Result<Member> {
        let project = Self::project_id(reference);
        let user_id = self.resolve_user_id(login).await?;
        let member: crate::models::MemberResponse = self
            .put_json(
                &format!("/projects/{project}/members/{user_id}"),
                &MemberUpdateRequest {
                    access_level: gitlab_access_level(&role),
                },
            )
            .await?;
        Ok(member.into())
    }

    async fn resolve_user_id(&self, login: &str) -> Result<u64> {
        let users: Vec<GitLabUserResponse> = self
            .get_json(&format!("/users?username={}", url_encode(login)))
            .await?;
        users
            .into_iter()
            .find(|user| user.username == login)
            .and_then(|user| user.id)
            .ok_or_else(|| ProviderError::NotFound {
                resource: format!("gitlab user {login}"),
                reference: None,
            })
    }
}

fn gitlab_access_level(role: &PermissionLevel) -> u32 {
    match role {
        PermissionLevel::Admin | PermissionLevel::Maintain => 40,
        PermissionLevel::Write => 30,
        PermissionLevel::Triage | PermissionLevel::Read => 20,
        PermissionLevel::None => 10,
    }
}
