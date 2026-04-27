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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommandInputMode {
    #[default]
    Structured,
    RawArgv,
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
    pub close_launcher_on_success: bool,
    #[serde(default)]
    pub input_mode: CommandInputMode,
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
    pub keywords: Vec<String>,
    #[serde(default)]
    pub starts_interactive_session: bool,
    #[serde(default)]
    pub close_launcher_on_success: bool,
    #[serde(default)]
    pub input_mode: CommandInputMode,
    #[serde(default)]
    pub arguments: Vec<CommandArgumentDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandExecutionRequest {
    pub command_id: CommandId,
    #[serde(default)]
    pub argv: Vec<String>,
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
    #[serde(default)]
    pub completion_behavior: InteractiveSessionCompletionBehavior,
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
    #[serde(default)]
    pub completion_behavior: InteractiveSessionCompletionBehavior,
    pub query: String,
    #[serde(default)]
    pub is_loading: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InteractiveSessionCompletionBehavior {
    #[default]
    HideLauncher,
    HideLauncherAndRestoreFocus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractiveSessionSubmitResult {
    UpdatedSession {
        session: InteractiveSessionState,
    },
    Completed {
        output: String,
        completion_behavior: InteractiveSessionCompletionBehavior,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

impl ThemePreference {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchResultKind {
    Command,
    Application,
    Bookmark,
    Image,
    BrowserTab,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAssetDefinition {
    pub id: CommandId,
    pub title: String,
    pub relative_path: String,
    pub path: String,
}

pub const BROWSER_TAB_COMMAND_PREFIX: &str = "browser-tab";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserTab {
    pub browser: String,
    pub window_id: String,
    pub window_index: u32,
    pub active_tab_index: u32,
    pub tab_index: u32,
    pub title: String,
    pub url: String,
}

impl BrowserTab {
    pub fn command_id(&self) -> CommandId {
        CommandId::from(format!(
            "{BROWSER_TAB_COMMAND_PREFIX}:{}:{}:{}",
            self.browser, self.window_id, self.tab_index
        ))
    }

    pub fn subtitle(&self) -> String {
        format!("{} · {}", self.browser_label(), self.url)
    }

    pub fn search_text(&self) -> String {
        format!("{} {} {}", self.title, self.url, self.browser).to_lowercase()
    }

    pub fn is_active(&self) -> bool {
        self.window_index == 1 && self.active_tab_index == self.tab_index
    }

    pub fn browser_label(&self) -> &str {
        match self.browser.as_str() {
            "chrome" => "Google Chrome",
            _ => self.browser.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserTabTarget {
    pub browser: String,
    pub window_id: String,
    pub tab_index: u32,
}

pub fn parse_browser_tab_command_id(command_id: &CommandId) -> Option<BrowserTabTarget> {
    let mut parts = command_id.as_str().split(':');
    let prefix = parts.next()?;
    let browser = parts.next()?;
    let window_id = parts.next()?;
    let tab_index = parts.next()?.parse::<u32>().ok()?;

    if prefix != BROWSER_TAB_COMMAND_PREFIX || parts.next().is_some() {
        return None;
    }

    Some(BrowserTabTarget {
        browser: browser.to_string(),
        window_id: window_id.to_string(),
        tab_index,
    })
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
pub struct SearchIndexStats {
    pub discovered_count: usize,
    pub indexed_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessMatch {
    pub pid: u32,
    pub display_name: String,
    pub executable_name: String,
    pub command: String,
    pub matched_ports: Vec<u16>,
}
