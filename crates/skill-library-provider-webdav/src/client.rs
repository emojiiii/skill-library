use percent_encoding::percent_decode_str;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Method, RequestBuilder, Url};
use skill_library_provider::{ProviderError, Result};

use crate::auth::WebDavAuth;
use crate::error::{provider_error_from_status, snippet};
use crate::paths::{collection_request_path, encode_path, normalize_repo_path_lossy};
use crate::propfind::{parse_propfind_response, DavEntry};
use crate::WebDavProvider;

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:resourcetype/>
    <D:getetag/>
    <D:getlastmodified/>
    <D:getcontentlength/>
  </D:prop>
</D:propfind>"#;

impl WebDavProvider {
    pub fn with_instance_base_url(
        instance_id: impl Into<String>,
        api_base: impl AsRef<str>,
        auth: Option<WebDavAuth>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("skill-library/0.1"));
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        let api_base = Url::parse(api_base.as_ref()).map_err(|err| {
            ProviderError::InvalidResponse(format!("invalid WebDAV base URL: {err}"))
        })?;
        Ok(Self {
            client,
            api_base,
            instance_id: instance_id.into(),
            auth,
        })
    }

    pub(crate) fn request(&self, method: Method, path: &str) -> Result<RequestBuilder> {
        let url = self.url_for(path)?;
        let builder = self.client.request(method, url);
        Ok(match self.auth.as_ref() {
            Some(WebDavAuth::Bearer(token)) => builder.bearer_auth(token),
            Some(WebDavAuth::Basic { username, password }) => {
                builder.basic_auth(username, Some(password))
            }
            None => builder,
        })
    }

    pub(crate) async fn propfind_collection(
        &self,
        collection_path: &str,
        depth: &str,
    ) -> Result<Vec<DavEntry>> {
        let request_path = collection_request_path(collection_path);
        let method = Method::from_bytes(b"PROPFIND")
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        let response = self
            .request(method, &request_path)?
            .header("Depth", depth)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(PROPFIND_BODY)
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| status.to_string());
            return Err(provider_error_from_status(
                status,
                format!("PROPFIND {collection_path} ({status}): {}", snippet(&body)),
            ));
        }
        let body = response
            .text()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        parse_propfind_response(self, collection_path, &body)
    }

    pub(crate) async fn get_bytes(&self, path: &str) -> Result<(HeaderMap, Vec<u8>)> {
        let response = self
            .request(Method::GET, path)?
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        let status = response.status();
        let headers = response.headers().clone();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| status.to_string());
            return Err(provider_error_from_status(
                status,
                format!("GET {path} ({status}): {}", snippet(&body)),
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

    pub(crate) fn url_for(&self, path: &str) -> Result<Url> {
        let mut url = self.api_base.clone();
        let base_path = url.path().trim_end_matches('/');
        let wants_trailing_slash = path.ends_with('/');
        let path = normalize_repo_path_lossy(path);
        let encoded = encode_path(&path);
        let final_path = if encoded.is_empty() {
            format!("{base_path}/")
        } else if wants_trailing_slash {
            format!("{base_path}/{encoded}/")
        } else {
            format!("{base_path}/{encoded}")
        };
        url.set_path(&final_path);
        Ok(url)
    }

    pub(crate) fn decoded_url_path(&self, path: &str) -> Result<String> {
        let url = self.url_for(path)?;
        Ok(percent_decode_str(url.path())
            .decode_utf8_lossy()
            .into_owned())
    }
}
