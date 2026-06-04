use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ChangedFile, Invitation, InvitationInput, IssueComment, Member, Page, PageOpts,
    PermissionLevel, PullRequest, PullRequestQueryState, PullRequestSummary, RepositoryEvent,
    Result,
};

use crate::models::{
    ClosePullRequestRequest, CollaboratorRequest, CollaboratorResponse, PullRequestCommentRequest,
    PullRequestCommentResponse, PullRequestFileResponse, PullRequestResponse,
    RepositoryEventResponse,
};
use crate::provider::GiteeProvider;
use crate::util::url_encode;

impl GiteeProvider {
    pub async fn list_pull_requests(
        &self,
        reference: &WorkspaceRef,
        state: PullRequestQueryState,
    ) -> Result<Vec<PullRequestSummary>> {
        let (owner, repo) = Self::owner_repo(reference);
        let state = match state {
            PullRequestQueryState::Open => "open",
            PullRequestQueryState::Closed => "closed",
            PullRequestQueryState::All => "all",
        };
        let raw: Vec<PullRequestResponse> = self
            .get_json(&format!(
                "/repos/{owner}/{repo}/pulls?state={state}&sort=updated&direction=desc&per_page=50"
            ))
            .await?;
        Ok(raw.into_iter().map(PullRequestSummary::from).collect())
    }

    pub async fn list_pull_request_files(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<Vec<ChangedFile>> {
        let (owner, repo) = Self::owner_repo(reference);
        let raw: Vec<PullRequestFileResponse> = self
            .get_json(&format!("/repos/{owner}/{repo}/pulls/{number}/files"))
            .await?;
        Ok(raw.into_iter().map(ChangedFile::from).collect())
    }

    pub async fn list_repository_events(
        &self,
        reference: &WorkspaceRef,
    ) -> Result<Vec<RepositoryEvent>> {
        let (owner, repo) = Self::owner_repo(reference);
        let raw: Vec<RepositoryEventResponse> = self
            .get_json(&format!("/repos/{owner}/{repo}/events?per_page=30"))
            .await?;
        Ok(raw.into_iter().map(RepositoryEvent::from).collect())
    }

    pub async fn list_collaborators(
        &self,
        reference: &WorkspaceRef,
        opts: PageOpts,
    ) -> Result<Page<Member>> {
        let (owner, repo) = Self::owner_repo(reference);
        let per_page = opts.per_page.unwrap_or(100);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let raw: Page<CollaboratorResponse> = self
            .get_page_json(&format!(
                "/repos/{owner}/{repo}/collaborators?per_page={per_page}&page={}",
                url_encode(&page)
            ))
            .await?;
        Ok(Page {
            items: raw.items.into_iter().map(Member::from).collect(),
            next_cursor: raw.next_cursor,
        })
    }

    pub async fn merge_pull_request(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<PullRequest> {
        let (owner, repo) = Self::owner_repo(reference);
        let raw: PullRequestResponse = self
            .put_json(
                &format!("/repos/{owner}/{repo}/pulls/{number}/merge"),
                &serde_json::json!({ "merge_method": "squash" }),
            )
            .await?;
        Ok(raw.into())
    }

    pub async fn close_pull_request(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<PullRequestSummary> {
        let (owner, repo) = Self::owner_repo(reference);
        let raw: PullRequestResponse = self
            .patch_json(
                &format!("/repos/{owner}/{repo}/pulls/{number}"),
                &ClosePullRequestRequest { state: "closed" },
            )
            .await?;
        Ok(raw.into())
    }

    pub async fn add_pull_request_comment(
        &self,
        reference: &WorkspaceRef,
        number: u64,
        body: &str,
    ) -> Result<IssueComment> {
        let (owner, repo) = Self::owner_repo(reference);
        let raw: PullRequestCommentResponse = self
            .post_json(
                &format!("/repos/{owner}/{repo}/pulls/{number}/comments"),
                &PullRequestCommentRequest { body },
            )
            .await?;
        Ok(raw.into())
    }

    pub async fn upsert_collaborator(
        &self,
        reference: &WorkspaceRef,
        input: InvitationInput,
    ) -> Result<Invitation> {
        let (owner, repo) = Self::owner_repo(reference);
        let permission = gitee_permission(&input.role);
        self.put_status(
            &format!(
                "/repos/{owner}/{repo}/collaborators/{}",
                url_encode(&input.login_or_email)
            ),
            &CollaboratorRequest { permission },
        )
        .await?;
        Ok(Invitation {
            id: input.login_or_email.clone(),
            login_or_email: input.login_or_email,
            state: "active".to_owned(),
        })
    }

    pub async fn update_collaborator_role(
        &self,
        reference: &WorkspaceRef,
        login: &str,
        role: PermissionLevel,
    ) -> Result<Member> {
        let assigned_role = role.clone();
        let invitation = self
            .upsert_collaborator(
                reference,
                InvitationInput {
                    login_or_email: login.to_owned(),
                    role,
                },
            )
            .await?;
        Ok(Member {
            login: invitation.login_or_email,
            role: assigned_role,
            avatar_url: None,
        })
    }
}

fn gitee_permission(role: &PermissionLevel) -> &'static str {
    match role {
        PermissionLevel::Admin => "admin",
        PermissionLevel::Maintain | PermissionLevel::Write => "push",
        PermissionLevel::Triage | PermissionLevel::Read | PermissionLevel::None => "pull",
    }
}
