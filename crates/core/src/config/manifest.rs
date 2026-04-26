use rayon_types::{CommandArgumentDefinition, CommandArgumentType, CommandArgumentValue};
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
pub(crate) struct ManifestCommand {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub program: String,
    pub base_args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub env: Option<BTreeMap<String, String>>,
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
pub(crate) struct ManifestArgument {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub argument_type: CommandArgumentType,
    #[serde(default)]
    pub required: bool,
    pub flag: Option<String>,
    pub positional: Option<usize>,
    pub default_string: Option<String>,
    pub default_boolean: Option<bool>,
}

impl From<ManifestArgument> for CommandArgumentDefinition {
    fn from(value: ManifestArgument) -> Self {
        let default_value = match value.argument_type {
            CommandArgumentType::String => value.default_string.map(CommandArgumentValue::String),
            CommandArgumentType::Boolean => {
                value.default_boolean.map(CommandArgumentValue::Boolean)
            }
        };

        Self {
            id: value.id,
            label: value.label,
            argument_type: value.argument_type,
            required: value.required,
            flag: value.flag,
            positional: value.positional,
            default_value,
        }
    }
}
