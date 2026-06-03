use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, USER_AGENT};
use serde::Deserialize;
use skill_library_core::{ProviderInstance, ProviderKind, WorkspaceRef};
use skill_library_provider::{Page, ProviderError, Result, SkillSourceProvider, SourceRef};

use crate::http::map_response;
use crate::util::url_encode;

const PRIVATE_TOKEN: HeaderName = HeaderName::from_static("private-token");

pub struct GitLabProvider {
    pub(crate) client: reqwest::Client,
    pub(crate) api_base: String,
    pub(crate) instance_id: String,
}

impl GitLabProvider {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        Self::with_instance_base_url(
            "gitlab.com",
            "https://gitlab.com/api/v4",
            Some(token.into()),
        )
    }

    pub fn anonymous(api_base: impl Into<String>) -> Result<Self> {
        Self::with_instance_base_url("gitlab.com", api_base, None)
    }

    pub fn for_instance(instance: &ProviderInstance, token: Option<String>) -> Result<Self> {
        if !matches!(instance.kind, ProviderKind::GitLab) {
            return Err(ProviderError::InvalidResponse(format!(
                "provider instance {} is not a GitLab provider",
                instance.id
            )));
        }
        Self::with_instance_base_url(instance.id.clone(), instance.api_base_url.clone(), token)
    }

    pub fn with_instance_base_url(
        instance_id: impl Into<String>,
        api_base: impl Into<String>,
        token: Option<String>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("skill-library/0.1"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(token) = token.filter(|token| !token.trim().is_empty()) {
            let value = HeaderValue::from_str(&token)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
            headers.insert(PRIVATE_TOKEN, value);
        }
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        Ok(Self {
            client,
            api_base: api_base.into().trim_end_matches('/').to_owned(),
            instance_id: instance_id.into(),
        })
    }

    pub(crate) async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitlab", method = "GET", path);
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        map_response(path, "GET", response).await
    }

    pub(crate) async fn get_page_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<Page<T>> {
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitlab", method = "GET", path);
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        let next_cursor = response
            .headers()
            .get("x-next-page")
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let items = map_response(path, "GET", response).await?;
        Ok(Page { items, next_cursor })
    }

    pub(crate) async fn get_bytes(&self, path: &str) -> Result<(HeaderMap, Vec<u8>)> {
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitlab", method = "GET", path);
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        let status = response.status();
        let headers = response.headers().clone();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| status.to_string());
            return Err(crate::http::provider_error_from_status(
                status,
                format!("GET {path} ({status}): {}", crate::util::snippet(&body)),
            ));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?
            .to_vec();
        Ok((headers, bytes))
    }

    pub(crate) async fn source_ref_value(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
    ) -> Result<String> {
        Ok(match at {
            SourceRef::Latest => {
                let workspace = self.get_source(reference).await?;
                if workspace.default_branch.trim().is_empty() {
                    "HEAD".to_owned()
                } else {
                    workspace.default_branch
                }
            }
            SourceRef::Version(version) => version.clone(),
            SourceRef::Git(git_ref) => git_ref.value().to_owned(),
            SourceRef::Revision(revision) => revision.clone(),
        })
    }

    pub(crate) fn project_id(reference: &WorkspaceRef) -> String {
        let value = reference
            .remote_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| reference.full_name());
        url_encode(&value)
    }
}
