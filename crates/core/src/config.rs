use crate::{CommandError, CommandProvider};
use rayon_types::{
    BookmarkDefinition, CommandArgumentDefinition, CommandArgumentType, CommandArgumentValue,
    CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use url::Url;

pub struct LoadedConfig {
    pub command_providers: Vec<Arc<dyn CommandProvider>>,
    pub bookmarks: Vec<BookmarkDefinition>,
}

pub fn load_config() -> Result<LoadedConfig, String> {
    let config_dir = config_dir()?;
    if !config_dir.exists() {
        return Ok(LoadedConfig {
            command_providers: Vec::new(),
            bookmarks: Vec::new(),
        });
    }

    let mut command_providers: Vec<Arc<dyn CommandProvider>> = Vec::new();
    let mut bookmarks = Vec::new();
    let mut bookmark_ids = HashSet::new();
    for manifest_path in manifest_paths(&config_dir)? {
        let manifest = load_manifest(&manifest_path)?;
        let LoadedManifest {
            command_provider,
            bookmarks: manifest_bookmarks,
        } = LoadedManifest::from_manifest(manifest_path.parent().unwrap_or(&config_dir), manifest)?;

        if !command_provider.is_empty() {
            command_providers.push(Arc::new(command_provider));
        }
        for bookmark in manifest_bookmarks {
            if !bookmark_ids.insert(bookmark.id.to_string()) {
                return Err(format!("duplicate bookmark id registered: {}", bookmark.id));
            }
            bookmarks.push(bookmark);
        }
    }

    Ok(LoadedConfig {
        command_providers,
        bookmarks,
    })
}

fn config_dir() -> Result<PathBuf, String> {
    if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg_config_home).join("rayon"));
    }

    let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".config").join("rayon"))
}

fn manifest_paths(config_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(config_dir).map_err(|error| {
        format!(
            "failed to read config directory {}: {error}",
            config_dir.display()
        )
    })?;

    let mut manifest_paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    manifest_paths.sort();
    Ok(manifest_paths)
}

fn load_manifest(path: &Path) -> Result<PluginManifest, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read plugin manifest {}: {error}", path.display()))?;
    toml::from_str(&source).map_err(|error| {
        format!(
            "failed to parse plugin manifest {}: {error}",
            path.display()
        )
    })
}

#[derive(Debug)]
struct DeclarativeCommandProvider {
    command_definitions: Vec<CommandDefinition>,
    commands_by_id: HashMap<String, ExecutableCommandSpec>,
}

impl DeclarativeCommandProvider {
    fn is_empty(&self) -> bool {
        self.command_definitions.is_empty()
    }
}

struct LoadedManifest {
    command_provider: DeclarativeCommandProvider,
    bookmarks: Vec<BookmarkDefinition>,
}

impl LoadedManifest {
    fn from_manifest(base_dir: &Path, manifest: PluginManifest) -> Result<Self, String> {
        let commands = manifest.commands.unwrap_or_default();
        let bookmarks = manifest.bookmarks.unwrap_or_default();
        let mut command_definitions = Vec::with_capacity(commands.len());
        let mut commands_by_id = HashMap::with_capacity(commands.len());
        let mut bookmark_definitions = Vec::with_capacity(bookmarks.len());

        for command in commands {
            let command_id = CommandId::from(command.id.clone());
            let definition = CommandDefinition {
                id: command_id.clone(),
                title: command.title.clone(),
                subtitle: command.subtitle.clone(),
                owner_plugin_id: manifest.plugin_id.clone(),
                keywords: command.keywords.clone().unwrap_or_default(),
                arguments: command
                    .arguments
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            };
            let spec =
                ExecutableCommandSpec::from_manifest_command(base_dir, &definition, command)?;

            command_definitions.push(definition);
            commands_by_id.insert(command_id.to_string(), spec);
        }

        for bookmark in bookmarks {
            validate_bookmark_url(&bookmark.url)?;
            bookmark_definitions.push(BookmarkDefinition {
                id: CommandId::from(bookmark.id),
                title: bookmark.title,
                subtitle: bookmark.subtitle,
                owner_plugin_id: manifest.plugin_id.clone(),
                url: bookmark.url,
                keywords: bookmark.keywords.unwrap_or_default(),
            });
        }

        Ok(Self {
            command_provider: DeclarativeCommandProvider {
                command_definitions,
                commands_by_id,
            },
            bookmarks: bookmark_definitions,
        })
    }
}

impl CommandProvider for DeclarativeCommandProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        self.command_definitions.clone()
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        let spec = self
            .commands_by_id
            .get(request.command_id.as_str())
            .ok_or_else(|| CommandError::UnknownCommand(request.command_id.clone()))?;

        spec.execute(request)
    }
}

#[derive(Debug)]
struct ExecutableCommandSpec {
    definition: CommandDefinition,
    program: PathBuf,
    base_args: Vec<String>,
    working_dir: Option<PathBuf>,
    env: BTreeMap<String, String>,
}

impl ExecutableCommandSpec {
    fn from_manifest_command(
        base_dir: &Path,
        definition: &CommandDefinition,
        command: ManifestCommand,
    ) -> Result<Self, String> {
        let program = resolve_path(base_dir, &command.program);
        let working_dir = command
            .working_dir
            .as_deref()
            .map(|path| resolve_path(base_dir, path));

        Ok(Self {
            definition: definition.clone(),
            program,
            base_args: command.base_args.unwrap_or_default(),
            working_dir,
            env: command.env.unwrap_or_default(),
        })
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        let mut argv = self.base_args.clone();
        let mut positional_values: BTreeMap<usize, String> = BTreeMap::new();

        for argument in &self.definition.arguments {
            let value = request
                .arguments
                .get(&argument.id)
                .cloned()
                .or_else(|| argument.default_value.clone());

            let Some(value) = value else {
                if argument.required {
                    return Err(CommandError::InvalidArguments(format!(
                        "missing required argument '{}'",
                        argument.label
                    )));
                }
                continue;
            };

            let encoded = encode_argument(argument, &value)?;
            if let Some(positional_index) = argument.positional {
                positional_values.insert(positional_index, encoded);
                continue;
            }

            if let Some(flag) = &argument.flag {
                match value {
                    CommandArgumentValue::Boolean(true) => argv.push(flag.clone()),
                    CommandArgumentValue::Boolean(false) => {}
                    CommandArgumentValue::String(_) => {
                        argv.push(flag.clone());
                        argv.push(encoded);
                    }
                }
                continue;
            }

            argv.push(encoded);
        }

        for (_, positional_value) in positional_values {
            argv.push(positional_value);
        }

        let mut command = Command::new(&self.program);
        command.args(&argv);
        if let Some(working_dir) = &self.working_dir {
            command.current_dir(working_dir);
        }
        if !self.env.is_empty() {
            command.envs(&self.env);
        }

        let output = command.output().map_err(|error| {
            CommandError::ExecutionFailed(format!(
                "failed to run {}: {error}",
                self.definition.title
            ))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let output_text = if !stdout.is_empty() {
            stdout
        } else if !stderr.is_empty() {
            stderr
        } else {
            format!("completed {}", self.definition.title)
        };

        if !output.status.success() {
            return Err(CommandError::ExecutionFailed(output_text));
        }

        Ok(CommandExecutionResult {
            output: output_text,
        })
    }
}

fn encode_argument(
    definition: &CommandArgumentDefinition,
    value: &CommandArgumentValue,
) -> Result<String, CommandError> {
    match (&definition.argument_type, value) {
        (CommandArgumentType::String, CommandArgumentValue::String(string_value)) => {
            Ok(string_value.clone())
        }
        (CommandArgumentType::Boolean, CommandArgumentValue::Boolean(bool_value)) => {
            Ok(bool_value.to_string())
        }
        (expected_type, actual_value) => Err(CommandError::InvalidArguments(format!(
            "argument '{}' expected {:?}, got {:?}",
            definition.label, expected_type, actual_value
        ))),
    }
}

fn resolve_path(base_dir: &Path, raw_path: &str) -> PathBuf {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

fn validate_bookmark_url(raw_url: &str) -> Result<(), String> {
    let parsed = Url::parse(raw_url)
        .map_err(|error| format!("invalid bookmark url '{raw_url}': {error}"))?;
    if parsed.scheme().is_empty() {
        return Err(format!(
            "invalid bookmark url '{raw_url}': missing URL scheme"
        ));
    }

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct PluginManifest {
    plugin_id: String,
    #[serde(default)]
    commands: Option<Vec<ManifestCommand>>,
    #[serde(default)]
    bookmarks: Option<Vec<ManifestBookmark>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestCommand {
    id: String,
    title: String,
    subtitle: Option<String>,
    keywords: Option<Vec<String>>,
    program: String,
    base_args: Option<Vec<String>>,
    working_dir: Option<String>,
    env: Option<BTreeMap<String, String>>,
    arguments: Option<Vec<ManifestArgument>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestBookmark {
    id: String,
    title: String,
    subtitle: Option<String>,
    url: String,
    keywords: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestArgument {
    id: String,
    label: String,
    #[serde(rename = "type")]
    argument_type: CommandArgumentType,
    #[serde(default)]
    required: bool,
    flag: Option<String>,
    positional: Option<usize>,
    default_string: Option<String>,
    default_boolean: Option<bool>,
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rayon-config-test-{suffix}"))
    }

    #[test]
    fn loads_toml_providers_from_xdg_path() {
        let _env_guard = env_lock().lock().unwrap();
        let config_home = unique_dir();
        let rayon_dir = config_home.join("rayon");
        fs::create_dir_all(&rayon_dir).unwrap();
        fs::write(
            rayon_dir.join("commands.toml"),
            r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"
base_args = ["hello"]
"#,
        )
        .unwrap();

        let previous = env::var_os("XDG_CONFIG_HOME");
        env::set_var("XDG_CONFIG_HOME", &config_home);
        let loaded = load_config().unwrap();
        if let Some(previous) = previous {
            env::set_var("XDG_CONFIG_HOME", previous);
        } else {
            env::remove_var("XDG_CONFIG_HOME");
        }

        assert_eq!(loaded.command_providers.len(), 1);
        let commands = loaded.command_providers[0].commands();
        assert_eq!(commands[0].owner_plugin_id, "user.commands");
    }

    #[test]
    fn loads_bookmarks_from_manifest() {
        let _env_guard = env_lock().lock().unwrap();
        let config_home = unique_dir();
        let rayon_dir = config_home.join("rayon");
        fs::create_dir_all(&rayon_dir).unwrap();
        fs::write(
            rayon_dir.join("bookmarks.toml"),
            r#"
plugin_id = "user.links"

[[bookmarks]]
id = "user.github"
title = "GitHub"
url = "https://github.com"
subtitle = "Code hosting"
keywords = ["git", "repos"]
"#,
        )
        .unwrap();

        let previous = env::var_os("XDG_CONFIG_HOME");
        env::set_var("XDG_CONFIG_HOME", &config_home);
        let loaded = load_config().unwrap();
        if let Some(previous) = previous {
            env::set_var("XDG_CONFIG_HOME", previous);
        } else {
            env::remove_var("XDG_CONFIG_HOME");
        }

        assert!(loaded.command_providers.is_empty());
        assert_eq!(loaded.bookmarks.len(), 1);
        assert_eq!(loaded.bookmarks[0].owner_plugin_id, "user.links");
        assert_eq!(loaded.bookmarks[0].url, "https://github.com");
    }

    #[test]
    fn rejects_invalid_bookmark_urls() {
        let _env_guard = env_lock().lock().unwrap();
        let config_home = unique_dir();
        let rayon_dir = config_home.join("rayon");
        fs::create_dir_all(&rayon_dir).unwrap();
        fs::write(
            rayon_dir.join("invalid.toml"),
            r#"
plugin_id = "user.links"

[[bookmarks]]
id = "user.bad"
title = "Bad"
url = "github.com"
"#,
        )
        .unwrap();

        let previous = env::var_os("XDG_CONFIG_HOME");
        env::set_var("XDG_CONFIG_HOME", &config_home);
        let result = load_config();
        if let Some(previous) = previous {
            env::set_var("XDG_CONFIG_HOME", previous);
        } else {
            env::remove_var("XDG_CONFIG_HOME");
        }

        assert!(matches!(result, Err(ref error) if error.contains("invalid bookmark url")));
    }
}
