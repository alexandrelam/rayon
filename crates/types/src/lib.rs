use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandId(pub String);

impl CommandId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for CommandId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CommandId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDefinition {
    pub id: CommandId,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: CommandId,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon_path: Option<String>,
    pub kind: SearchResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandExecutionResult {
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchResultKind {
    Command,
    Application,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledApp {
    pub id: CommandId,
    pub title: String,
    pub bundle_identifier: Option<String>,
    pub path: String,
}

impl InstalledApp {
    pub fn subtitle(&self) -> String {
        self.bundle_identifier
            .clone()
            .unwrap_or_else(|| self.path.clone())
    }

    pub fn search_text(&self) -> String {
        let mut parts = vec![self.title.clone()];
        if let Some(bundle_identifier) = &self.bundle_identifier {
            parts.push(bundle_identifier.clone());
        }
        if let Some(bundle_name) = std::path::Path::new(&self.path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|name| !name.is_empty())
        {
            parts.push(bundle_name.to_string());
        }

        parts.join(" ")
    }
}
