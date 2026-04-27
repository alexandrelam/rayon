use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PluginManifest {
    pub plugin_id: String,
    #[serde(default)]
    pub commands: Option<Vec<ManifestCommand>>,
    #[serde(default)]
    pub bookmarks: Option<Vec<ManifestBookmark>>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ManifestCommand {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub keywords: Option<Vec<String>>,
    #[serde(default)]
    pub close_launcher_on_success: bool,
    #[serde(default)]
    pub input_mode: Option<String>,
    pub program: String,
    pub base_args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub arguments: Option<Vec<ManifestArgument>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ManifestBookmark {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub url: String,
    pub keywords: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ManifestArgument {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub argument_type: String,
    #[serde(default)]
    pub required: bool,
    pub flag: Option<String>,
    pub positional: Option<usize>,
    pub default_string: Option<String>,
    pub default_boolean: Option<bool>,
}
