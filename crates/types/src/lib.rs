use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
#[serde(rename_all = "snake_case")]
pub enum CommandArgumentType {
    String,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum CommandArgumentValue {
    String(String),
    Boolean(bool),
}

impl CommandArgumentValue {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            Self::Boolean(_) => None,
        }
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Self::String(_) => None,
            Self::Boolean(value) => Some(*value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandArgumentDefinition {
    pub id: String,
    pub label: String,
    pub argument_type: CommandArgumentType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub flag: Option<String>,
    #[serde(default)]
    pub positional: Option<usize>,
    #[serde(default)]
    pub default_value: Option<CommandArgumentValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDefinition {
    pub id: CommandId,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    pub owner_plugin_id: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub arguments: Vec<CommandArgumentDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookmarkDefinition {
    pub id: CommandId,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    pub owner_plugin_id: String,
    pub url: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: CommandId,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon_path: Option<String>,
    pub kind: SearchResultKind,
    pub owner_plugin_id: Option<String>,
    #[serde(default)]
    pub arguments: Vec<CommandArgumentDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandExecutionRequest {
    pub command_id: CommandId,
    #[serde(default)]
    pub arguments: HashMap<String, CommandArgumentValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandExecutionResult {
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveSessionMetadata {
    pub session_id: String,
    pub command_id: CommandId,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    pub input_placeholder: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveSessionResult {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveSessionState {
    pub session_id: String,
    pub command_id: CommandId,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    pub input_placeholder: String,
    pub query: String,
    #[serde(default)]
    pub results: Vec<InteractiveSessionResult>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveSessionQueryRequest {
    pub session_id: String,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveSessionSubmitRequest {
    pub session_id: String,
    pub query: String,
    pub item_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandInvocationResult {
    Completed { output: String },
    StartedSession { session: InteractiveSessionState },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchResultKind {
    Command,
    Application,
    Bookmark,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchableItemDocument {
    pub id: CommandId,
    pub kind: SearchResultKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub owner_plugin_id: Option<String>,
    pub search_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessMatch {
    pub pid: u32,
    pub display_name: String,
    pub executable_name: String,
    pub command: String,
    pub matched_ports: Vec<u16>,
}
