use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use skill_library_core::{ProviderInstance, ProviderKind, WorkspaceRef};
use skill_library_provider::{Page, ProviderError, Result, SkillSourceProvider, SourceRef};

use crate::http::map_response;
use crate::util::{redact_access_token, snippet, url_encode};

pub struct GiteeProvider {
    pub(crate) client: reqwest::Client,
    pub(crate) api_base: String,
    pub(crate) instance_id: String,
    pub(crate) token: Option<String>,
}

impl GiteeProvider {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        Self::with_instance_base_url("gitee.com", "https://gitee.com/api/v5", Some(token.into()))
    }

    pub fn anonymous(api_base: impl Into<String>) -> Result<Self> {
        Self::with_instance_base_url("gitee.com", api_base, None)
    }

    pub fn for_instance(instance: &ProviderInstance, token: Option<String>) -> Result<Self> {
        if !matches!(instance.kind, ProviderKind::Gitee) {
            return Err(ProviderError::InvalidResponse(format!(
                "provider instance {} is not a Gitee provider",
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
        if let Some(token) = token.as_ref().filter(|token| !token.trim().is_empty()) {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?,
            );
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
            token: token.filter(|token| !token.trim().is_empty()),
        })
    }

    pub(crate) async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let path = self.auth_path(path);
        let safe_path = redact_access_token(&path);
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitee", method = "GET", path = %safe_path);
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        map_response(&safe_path, "GET", response).await
    }

    pub(crate) async fn post_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let path = self.auth_path(path);
        let safe_path = redact_access_token(&path);
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitee", method = "POST", path = %safe_path);
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        map_response(&safe_path, "POST", response).await
    }

    pub(crate) async fn put_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let path = self.auth_path(path);
        let safe_path = redact_access_token(&path);
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitee", method = "PUT", path = %safe_path);
        let response = self
            .client
            .put(url)
            .json(body)
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        map_response(&safe_path, "PUT", response).await
    }

    pub(crate) async fn put_status<B: Serialize>(&self, path: &str, body: &B) -> Result<()> {
        let path = self.auth_path(path);
        let safe_path = redact_access_token(&path);
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitee", method = "PUT", path = %safe_path);
        let response = self
            .client
            .put(url)
            .json(body)
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        self.map_empty_response(&safe_path, "PUT", response).await
    }

    pub(crate) async fn patch_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let path = self.auth_path(path);
        let safe_path = redact_access_token(&path);
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitee", method = "PATCH", path = %safe_path);
        let response = self
            .client
            .patch(url)
            .json(body)
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        map_response(&safe_path, "PATCH", response).await
    }

    async fn map_empty_response(
        &self,
        path: &str,
        method: &str,
        response: reqwest::Response,
    ) -> Result<()> {
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_else(|_| status.to_string());
        let body = snippet(&body);
        tracing::warn!(
            target: "skill-library-gitee",
            method,
            path,
            status = status.as_u16(),
            body = %body,
            "non-success response"
        );
        Err(crate::http::provider_error_from_status(
            status,
            format!("{method} {path} ({status}): {body}"),
        ))
    }

    pub(crate) async fn get_page_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<Page<T>> {
        let path = self.auth_path(path);
        let safe_path = redact_access_token(&path);
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitee", method = "GET", path = %safe_path);
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
        let items = map_response(&safe_path, "GET", response).await?;
        Ok(Page { items, next_cursor })
    }

    pub(crate) async fn get_bytes(&self, path: &str) -> Result<(HeaderMap, Vec<u8>)> {
        let path = self.auth_path(path);
        let safe_path = redact_access_token(&path);
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitee", method = "GET", path = %safe_path);
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
            let body = snippet(&body);
            tracing::warn!(
                target: "skill-library-gitee",
                method = "GET",
                path = %safe_path,
                status = status.as_u16(),
                body = %body,
                "non-success response"
            );
            return Err(crate::http::provider_error_from_status(
                status,
                format!("GET {safe_path} ({status}): {body}"),
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

    pub(crate) fn auth_path(&self, path: &str) -> String {
        let Some(token) = self.token.as_ref() else {
            return path.to_owned();
        };
        let separator = if path.contains('?') { '&' } else { '?' };
        format!("{path}{separator}access_token={}", url_encode(token))
    }

    pub(crate) fn owner_repo(reference: &WorkspaceRef) -> (String, String) {
        (url_encode(&reference.owner), url_encode(&reference.repo))
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
                    "master".to_owned()
                } else {
                    workspace.default_branch
                }
            }
            SourceRef::Version(version) => version.clone(),
            SourceRef::Git(git_ref) => git_ref.value().to_owned(),
            SourceRef::Revision(revision) => revision.clone(),
        })
    }
}
