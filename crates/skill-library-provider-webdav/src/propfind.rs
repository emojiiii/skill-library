use percent_encoding::percent_decode_str;
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::Url;
use skill_library_provider::{ProviderError, Result};

use crate::paths::{normalize_repo_path_lossy, normalize_url_path};
use crate::WebDavProvider;

#[derive(Debug, Clone, Default)]
pub(crate) struct DavEntry {
    pub(crate) relative_path: String,
    pub(crate) is_collection: bool,
    pub(crate) etag: Option<String>,
    pub(crate) last_modified: Option<String>,
    pub(crate) content_length: Option<u64>,
}

impl DavEntry {
    pub(crate) fn stable_id(&self) -> String {
        self.etag
            .clone()
            .or_else(|| self.last_modified.clone())
            .unwrap_or_default()
    }
}

#[derive(Default)]
struct RawDavResponse {
    href: Option<String>,
    is_collection: bool,
    etag: Option<String>,
    last_modified: Option<String>,
    content_length: Option<u64>,
}

pub(crate) fn parse_propfind_response(
    provider: &WebDavProvider,
    collection_path: &str,
    body: &str,
) -> Result<Vec<DavEntry>> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut current: Option<RawDavResponse> = None;
    let mut current_field: Option<Field> = None;
    let mut entries = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => match local_name(event.local_name().as_ref()) {
                "response" => current = Some(RawDavResponse::default()),
                "href" => current_field = Some(Field::Href),
                "getetag" => current_field = Some(Field::Etag),
                "getlastmodified" => current_field = Some(Field::LastModified),
                "getcontentlength" => current_field = Some(Field::ContentLength),
                "collection" => {
                    if let Some(current) = current.as_mut() {
                        current.is_collection = true;
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(event)) => {
                if local_name(event.local_name().as_ref()) == "collection" {
                    if let Some(current) = current.as_mut() {
                        current.is_collection = true;
                    }
                }
            }
            Ok(Event::Text(text)) => {
                let Some(field) = current_field else {
                    buf.clear();
                    continue;
                };
                let value = text
                    .decode()
                    .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?
                    .trim()
                    .to_owned();
                if let Some(current) = current.as_mut() {
                    match field {
                        Field::Href => current.href = Some(value),
                        Field::Etag => current.etag = Some(value),
                        Field::LastModified => current.last_modified = Some(value),
                        Field::ContentLength => current.content_length = value.parse::<u64>().ok(),
                    }
                }
            }
            Ok(Event::End(event)) => match local_name(event.local_name().as_ref()) {
                "response" => {
                    if let Some(raw) = current.take() {
                        if let Some(entry) = raw.into_entry(provider, collection_path)? {
                            entries.push(entry);
                        }
                    }
                }
                "href" | "getetag" | "getlastmodified" | "getcontentlength" => {
                    current_field = None;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(ProviderError::InvalidResponse(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(entries)
}

impl RawDavResponse {
    fn into_entry(
        self,
        provider: &WebDavProvider,
        collection_path: &str,
    ) -> Result<Option<DavEntry>> {
        let Some(href) = self.href else {
            return Ok(None);
        };
        let Some(relative_path) = relative_href_path(provider, collection_path, &href)? else {
            return Ok(None);
        };
        Ok(Some(DavEntry {
            relative_path,
            is_collection: self.is_collection,
            etag: self.etag,
            last_modified: self.last_modified,
            content_length: self.content_length,
        }))
    }
}

#[derive(Debug, Clone, Copy)]
enum Field {
    Href,
    Etag,
    LastModified,
    ContentLength,
}

fn relative_href_path(
    provider: &WebDavProvider,
    collection_path: &str,
    href: &str,
) -> Result<Option<String>> {
    let href_path = decoded_href_path(&provider.api_base, href);
    let root_path = normalize_url_path(&provider.decoded_url_path(collection_path)?);
    let href_path = normalize_url_path(&href_path);
    if href_path == root_path {
        return Ok(Some(String::new()));
    }
    let root_prefix = format!("{}/", root_path.trim_end_matches('/'));
    if let Some(relative) = href_path.strip_prefix(&root_prefix) {
        return Ok(Some(normalize_repo_path_lossy(relative)));
    }
    Ok(None)
}

fn decoded_href_path(base: &Url, href: &str) -> String {
    let raw_path = base
        .join(href)
        .ok()
        .map(|url| url.path().to_owned())
        .unwrap_or_else(|| href.split(['?', '#']).next().unwrap_or(href).to_owned());
    percent_decode_str(&raw_path)
        .decode_utf8_lossy()
        .into_owned()
}

fn local_name(name: &[u8]) -> &str {
    let raw = std::str::from_utf8(name).unwrap_or_default();
    raw.rsplit_once(':').map(|(_, name)| name).unwrap_or(raw)
}
