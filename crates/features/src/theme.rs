use rayon_core::{
    CommandError, CommandProvider, InteractiveSessionSubmitOutcome, InteractiveSessionUpdate,
};
use rayon_types::{
    CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId,
    CommandInputMode, InteractiveSessionCompletionBehavior, InteractiveSessionMetadata,
    InteractiveSessionResult, ThemePreference,
};
use std::sync::Arc;

const THEME_COMMAND_ID: &str = "theme.set";

pub struct ThemeCommandProvider {
    settings: Arc<ThemeSettingsStore>,
}

impl ThemeCommandProvider {
    pub fn new(settings: Arc<ThemeSettingsStore>) -> Self {
        Self { settings }
    }
}

impl CommandProvider for ThemeCommandProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from(THEME_COMMAND_ID),
            title: "Set Theme".into(),
            subtitle: Some("Choose light, dark, or system appearance".into()),
            owner_plugin_id: "builtin.theme".into(),
            keywords: vec![
                "theme".into(),
                "appearance".into(),
                "light".into(),
                "dark".into(),
                "system".into(),
            ],
            close_launcher_on_success: false,
            input_mode: CommandInputMode::Structured,
            arguments: Vec::new(),
        }]
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        Err(CommandError::UnknownCommand(request.command_id.clone()))
    }

    fn start_interactive_session(
        &self,
        command_id: &CommandId,
    ) -> Result<Option<InteractiveSessionMetadata>, CommandError> {
        if command_id.as_str() != THEME_COMMAND_ID {
            return Ok(None);
        }

        Ok(Some(InteractiveSessionMetadata {
            session_id: String::new(),
            command_id: command_id.clone(),
            title: "Set Theme".into(),
            subtitle: Some("Choose the launcher appearance".into()),
            input_placeholder: "Filter light, dark, or system".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        }))
    }

    fn search_interactive_session(
        &self,
        session: &InteractiveSessionMetadata,
        query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        if session.command_id.as_str() != THEME_COMMAND_ID {
            return Err(CommandError::UnknownCommand(session.command_id.clone()));
        }

        let normalized_query = query.trim().to_lowercase();
        Ok(theme_options()
            .into_iter()
            .filter(|option| {
                normalized_query.is_empty()
                    || option.title.to_lowercase().contains(&normalized_query)
                    || option
                        .subtitle
                        .as_deref()
                        .is_some_and(|subtitle| subtitle.to_lowercase().contains(&normalized_query))
                    || option.id.contains(&normalized_query)
            })
            .collect())
    }

    fn submit_interactive_session(
        &self,
        session: &InteractiveSessionMetadata,
        query: &str,
        item_id: &str,
    ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
        if session.command_id.as_str() != THEME_COMMAND_ID {
            return Err(CommandError::UnknownCommand(session.command_id.clone()));
        }

        let preference = ThemePreference::parse(item_id)
            .ok_or_else(|| CommandError::InvalidArguments(format!("unknown theme: {item_id}")))?;
        self.settings
            .save(preference)
            .map_err(CommandError::ExecutionFailed)?;

        let message = match preference {
            ThemePreference::Light => "theme set to light",
            ThemePreference::Dark => "theme set to dark",
            ThemePreference::System => "theme set to system",
        };

        let results = self.search_interactive_session(session, query)?;
        Ok(InteractiveSessionSubmitOutcome::Updated(
            InteractiveSessionUpdate {
                results,
                message: Some(message.into()),
            },
        ))
    }
}

fn theme_options() -> Vec<InteractiveSessionResult> {
    vec![
        InteractiveSessionResult {
            id: "light".into(),
            title: "Light".into(),
            subtitle: Some("Always use the light theme".into()),
        },
        InteractiveSessionResult {
            id: "dark".into(),
            title: "Dark".into(),
            subtitle: Some("Always use the dark theme".into()),
        },
        InteractiveSessionResult {
            id: "system".into(),
            title: "System".into(),
            subtitle: Some("Follow the current OS appearance".into()),
        },
    ]
}

#[derive(Clone)]
pub struct ThemeSettingsStore {
    path: std::path::PathBuf,
}

impl ThemeSettingsStore {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<ThemePreference, String> {
        match self.load_file() {
            Ok(settings) => Ok(settings.theme),
            Err(error) => {
                eprintln!("failed to parse theme settings: {error}");
                Ok(ThemePreference::System)
            }
        }
    }

    pub fn save(&self, theme: ThemePreference) -> Result<(), String> {
        let settings = ThemeSettingsFile { theme };
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let contents = serde_json::to_vec_pretty(&settings).map_err(|error| error.to_string())?;
        std::fs::write(&self.path, contents).map_err(|error| error.to_string())
    }

    fn load_file(&self) -> Result<ThemeSettingsFile, String> {
        match std::fs::read(&self.path) {
            Ok(contents) => serde_json::from_slice(&contents).map_err(|error| error.to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ThemeSettingsFile {
                theme: ThemePreference::System,
            }),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ThemeSettingsFile {
    #[serde(default)]
    theme: ThemePreference,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rayon-theme-{test_name}-{unique}.json"))
    }

    #[test]
    fn settings_store_defaults_to_system_when_missing() {
        let store = ThemeSettingsStore::new(temp_path("missing"));
        assert_eq!(store.load().unwrap(), ThemePreference::System);
    }

    #[test]
    fn settings_store_saves_and_loads_preference() {
        let path = temp_path("save-load");
        let store = ThemeSettingsStore::new(path.clone());
        store.save(ThemePreference::Dark).unwrap();

        assert_eq!(store.load().unwrap(), ThemePreference::Dark);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn settings_store_returns_error_for_invalid_file() {
        let path = temp_path("invalid");
        std::fs::write(&path, "{not-json").unwrap();
        let store = ThemeSettingsStore::new(path.clone());

        assert_eq!(store.load().unwrap(), ThemePreference::System);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn theme_provider_registers_command_and_options() {
        let store = Arc::new(ThemeSettingsStore::new(temp_path("provider-options")));
        let provider = ThemeCommandProvider::new(store);
        let command = provider.commands().pop().unwrap();

        assert_eq!(command.id, CommandId::from(THEME_COMMAND_ID));

        let session = provider
            .start_interactive_session(&CommandId::from(THEME_COMMAND_ID))
            .unwrap()
            .unwrap();
        let results = provider.search_interactive_session(&session, "").unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, "light");
        assert_eq!(results[1].id, "dark");
        assert_eq!(results[2].id, "system");
    }

    #[test]
    fn theme_provider_persists_selected_preference() {
        let path = temp_path("provider-submit");
        let store = Arc::new(ThemeSettingsStore::new(path.clone()));
        let provider = ThemeCommandProvider::new(store.clone());
        let session = provider
            .start_interactive_session(&CommandId::from(THEME_COMMAND_ID))
            .unwrap()
            .unwrap();

        provider
            .submit_interactive_session(&session, "", "light")
            .unwrap();

        assert_eq!(store.load().unwrap(), ThemePreference::Light);

        let _ = std::fs::remove_file(path);
    }
}
