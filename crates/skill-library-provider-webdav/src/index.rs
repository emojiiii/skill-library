use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::paths::{join_repo_path, normalize_repo_path_lossy};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebDavIndex {
    #[serde(default)]
    pub schema_version: Option<u32>,
    #[serde(default)]
    pub skills: Vec<WebDavIndexSkill>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebDavIndexSkill {
    pub id: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub latest: Option<String>,
    #[serde(default)]
    pub versions: BTreeMap<String, String>,
    #[serde(default)]
    pub checksum: Option<String>,
}

impl WebDavIndexSkill {
    pub fn display_path(&self) -> String {
        self.path
            .as_deref()
            .map(normalize_repo_path_lossy)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| normalize_repo_path_lossy(&self.id))
    }

    pub fn dir_for_ref(&self, version: Option<&str>) -> Option<String> {
        let raw = match version.map(str::trim).filter(|value| !value.is_empty()) {
            Some("latest") | None => self.latest.as_deref().or_else(|| {
                self.versions
                    .iter()
                    .next_back()
                    .map(|(_, path)| path.as_str())
            })?,
            Some(version) => self.versions.get(version).map(String::as_str)?,
        };
        Some(self.resolve_index_path(raw))
    }

    fn resolve_index_path(&self, value: &str) -> String {
        let value = normalize_repo_path_lossy(value);
        if value.is_empty() {
            return self.display_path();
        }
        let root = self.display_path();
        if value == root || value.starts_with(&format!("{root}/")) {
            value
        } else {
            join_repo_path(&root, &value)
        }
    }
}
