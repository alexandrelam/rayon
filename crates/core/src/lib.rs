use rayon_db::{AppIndexStats, SonicAppIndex};
use rayon_platform::MacOsAppManager;
use rayon_types::{
    CommandDefinition, CommandExecutionResult, CommandId, InstalledApp, SearchResult,
    SearchResultKind,
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock};

pub const APP_REINDEX_COMMAND_ID: &str = "apps.reindex";
const APP_SEARCH_LIMIT: usize = 20;

pub trait CommandProvider: Send + Sync {
    fn commands(&self) -> Vec<CommandDefinition>;
    fn execute(
        &self,
        command_id: &CommandId,
        payload: Option<String>,
    ) -> Result<CommandExecutionResult, CommandError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    DuplicateCommandId(CommandId),
    UnknownCommand(CommandId),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCommandId(command_id) => {
                write!(f, "duplicate command id registered: {command_id}")
            }
            Self::UnknownCommand(command_id) => write!(f, "unknown command id: {command_id}"),
        }
    }
}

impl Error for CommandError {}

#[derive(Debug)]
pub enum LauncherError {
    Command(CommandError),
    AppNotFound(CommandId),
    Platform(String),
    SearchBackend(String),
}

impl fmt::Display for LauncherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => write!(f, "{error}"),
            Self::AppNotFound(command_id) => write!(f, "unknown application id: {command_id}"),
            Self::Platform(error) => write!(f, "{error}"),
            Self::SearchBackend(error) => write!(f, "{error}"),
        }
    }
}

impl Error for LauncherError {}

impl From<CommandError> for LauncherError {
    fn from(value: CommandError) -> Self {
        Self::Command(value)
    }
}

pub struct CommandRegistry {
    providers: Vec<Arc<dyn CommandProvider>>,
    commands: Vec<RegisteredCommand>,
    command_owners: HashMap<String, usize>,
}

struct RegisteredCommand {
    definition: CommandDefinition,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            commands: Vec::new(),
            command_owners: HashMap::new(),
        }
    }

    pub fn register_provider(
        &mut self,
        provider: Arc<dyn CommandProvider>,
    ) -> Result<(), CommandError> {
        let provider_index = self.providers.len();
        let definitions = provider.commands();

        for definition in definitions {
            let command_key = definition.id.to_string();
            if self.command_owners.contains_key(&command_key) {
                return Err(CommandError::DuplicateCommandId(definition.id));
            }

            self.command_owners.insert(command_key, provider_index);
            self.commands.push(RegisteredCommand { definition });
        }

        self.providers.push(provider);
        Ok(())
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query = query.trim().to_ascii_lowercase();

        self.commands
            .iter()
            .filter(|command| {
                if query.is_empty() {
                    return true;
                }

                let title = command.definition.title.to_ascii_lowercase();
                let id = command.definition.id.as_str().to_ascii_lowercase();

                title.contains(&query) || id.contains(&query)
            })
            .map(|command| SearchResult {
                id: command.definition.id.clone(),
                title: command.definition.title.clone(),
                subtitle: None,
                icon_path: None,
                kind: SearchResultKind::Command,
            })
            .collect()
    }

    pub fn execute(
        &self,
        command_id: &CommandId,
        payload: Option<String>,
    ) -> Result<CommandExecutionResult, CommandError> {
        let provider_index = self
            .command_owners
            .get(command_id.as_str())
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(command_id.clone()))?;

        self.providers[provider_index].execute(command_id, payload)
    }
}

pub trait AppPlatform: Send + Sync {
    fn discover_apps(&self) -> Result<Vec<InstalledApp>, String>;
    fn launch_app(&self, app: &InstalledApp) -> Result<(), String>;
}

pub trait AppIndex: Send + Sync {
    fn is_configured(&self) -> bool;
    fn search_app_ids(&self, query: &str, limit: usize) -> Result<Vec<String>, String>;
    fn reindex_apps(&self, apps: &[InstalledApp]) -> Result<AppIndexStats, String>;
}

impl AppPlatform for MacOsAppManager {
    fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
        MacOsAppManager::discover_apps(self)
    }

    fn launch_app(&self, app: &InstalledApp) -> Result<(), String> {
        MacOsAppManager::launch_app(self, app)
    }
}

impl AppIndex for SonicAppIndex {
    fn is_configured(&self) -> bool {
        SonicAppIndex::is_configured(self)
    }

    fn search_app_ids(&self, query: &str, limit: usize) -> Result<Vec<String>, String> {
        SonicAppIndex::search_app_ids(self, query, limit).map_err(|error| error.to_string())
    }

    fn reindex_apps(&self, apps: &[InstalledApp]) -> Result<AppIndexStats, String> {
        SonicAppIndex::reindex_apps(self, apps).map_err(|error| error.to_string())
    }
}

#[derive(Default)]
struct AppCatalog {
    by_id: HashMap<String, InstalledApp>,
}

impl AppCatalog {
    fn from_apps(apps: Vec<InstalledApp>) -> Self {
        let mut by_id = HashMap::new();
        for app in apps {
            by_id.insert(app.id.to_string(), app);
        }

        Self { by_id }
    }

    fn get(&self, app_id: &CommandId) -> Option<&InstalledApp> {
        self.by_id.get(app_id.as_str())
    }
}

pub struct LauncherService {
    registry: CommandRegistry,
    platform: Arc<dyn AppPlatform>,
    app_index: Arc<dyn AppIndex>,
    app_catalog: RwLock<AppCatalog>,
}

impl LauncherService {
    pub fn new(
        registry: CommandRegistry,
        platform: Arc<dyn AppPlatform>,
        app_index: Arc<dyn AppIndex>,
    ) -> Self {
        let app_catalog = match platform.discover_apps() {
            Ok(apps) => AppCatalog::from_apps(apps),
            Err(error) => {
                eprintln!("failed to discover apps on startup: {error}");
                AppCatalog::default()
            }
        };

        Self {
            registry,
            platform,
            app_index,
            app_catalog: RwLock::new(app_catalog),
        }
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let mut results = self.registry.search(query);
        let app_ids = match self.app_index.search_app_ids(query, APP_SEARCH_LIMIT) {
            Ok(app_ids) => app_ids,
            Err(error) => {
                eprintln!("app search failed: {error}");
                Vec::new()
            }
        };

        let app_catalog = self.app_catalog.read().expect("app catalog lock poisoned");
        for app_id in app_ids {
            if let Some(app) = app_catalog.by_id.get(&app_id) {
                results.push(SearchResult {
                    id: app.id.clone(),
                    title: app.title.clone(),
                    subtitle: Some(app.subtitle()),
                    icon_path: None,
                    kind: SearchResultKind::Application,
                });
            }
        }

        results
    }

    pub fn execute(
        &self,
        command_id: &CommandId,
        payload: Option<String>,
    ) -> Result<CommandExecutionResult, LauncherError> {
        if command_id.as_str() == APP_REINDEX_COMMAND_ID {
            return self.refresh_and_reindex();
        }

        if command_id.as_str().starts_with("app:macos:") {
            return self.launch_app(command_id);
        }

        self.registry
            .execute(command_id, payload)
            .map_err(LauncherError::from)
    }

    pub fn app_search_enabled(&self) -> bool {
        self.app_index.is_configured()
    }

    fn refresh_and_reindex(&self) -> Result<CommandExecutionResult, LauncherError> {
        let apps = self
            .platform
            .discover_apps()
            .map_err(LauncherError::Platform)?;
        {
            let mut app_catalog = self.app_catalog.write().expect("app catalog lock poisoned");
            *app_catalog = AppCatalog::from_apps(apps.clone());
        }

        let stats = self
            .app_index
            .reindex_apps(&apps)
            .map_err(LauncherError::SearchBackend)?;

        Ok(CommandExecutionResult {
            output: format!(
                "reindexed {} apps into Sonic ({} skipped)",
                stats.indexed_count, stats.skipped_count
            ),
        })
    }

    fn launch_app(&self, command_id: &CommandId) -> Result<CommandExecutionResult, LauncherError> {
        let app = {
            let app_catalog = self.app_catalog.read().expect("app catalog lock poisoned");
            app_catalog
                .get(command_id)
                .cloned()
                .ok_or_else(|| LauncherError::AppNotFound(command_id.clone()))?
        };

        self.platform
            .launch_app(&app)
            .map_err(LauncherError::Platform)?;

        Ok(CommandExecutionResult {
            output: format!("opened {}", app.title),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestProvider;

    impl CommandProvider for TestProvider {
        fn commands(&self) -> Vec<CommandDefinition> {
            vec![
                CommandDefinition {
                    id: CommandId::from("hello"),
                    title: "Hello".into(),
                },
                CommandDefinition {
                    id: CommandId::from("help"),
                    title: "Help".into(),
                },
            ]
        }

        fn execute(
            &self,
            command_id: &CommandId,
            _payload: Option<String>,
        ) -> Result<CommandExecutionResult, CommandError> {
            Ok(CommandExecutionResult {
                output: format!("ran:{command_id}"),
            })
        }
    }

    #[test]
    fn returns_all_commands_for_empty_query() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let results = registry.search("");

        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|result| result.kind == SearchResultKind::Command));
    }

    #[test]
    fn filters_commands_case_insensitively() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let results = registry.search("HEL");

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn rejects_duplicate_command_ids() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let error = registry
            .register_provider(Arc::new(TestProvider))
            .unwrap_err();

        assert_eq!(
            error,
            CommandError::DuplicateCommandId(CommandId::from("hello"))
        );
    }

    #[test]
    fn executes_command_through_provider() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let result = registry.execute(&CommandId::from("hello"), None).unwrap();

        assert_eq!(result.output, "ran:hello");
    }

    #[test]
    fn returns_error_for_unknown_command() {
        let registry = CommandRegistry::new();

        let error = registry
            .execute(&CommandId::from("missing"), None)
            .unwrap_err();

        assert_eq!(
            error,
            CommandError::UnknownCommand(CommandId::from("missing"))
        );
    }

    struct StubPlatform {
        apps: Vec<InstalledApp>,
        launched: Mutex<Vec<String>>,
    }

    impl AppPlatform for StubPlatform {
        fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
            Ok(self.apps.clone())
        }

        fn launch_app(&self, app: &InstalledApp) -> Result<(), String> {
            self.launched.lock().unwrap().push(app.id.to_string());
            Ok(())
        }
    }

    struct StubIndex {
        configured: bool,
        search_results: Vec<String>,
        stats: AppIndexStats,
    }

    impl AppIndex for StubIndex {
        fn is_configured(&self) -> bool {
            self.configured
        }

        fn search_app_ids(&self, _query: &str, _limit: usize) -> Result<Vec<String>, String> {
            Ok(self.search_results.clone())
        }

        fn reindex_apps(&self, apps: &[InstalledApp]) -> Result<AppIndexStats, String> {
            Ok(AppIndexStats {
                discovered_count: apps.len(),
                ..self.stats.clone()
            })
        }
    }

    fn build_launcher_service(search_results: Vec<String>) -> LauncherService {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let platform = Arc::new(StubPlatform {
            apps: vec![InstalledApp {
                id: CommandId::from("app:macos:com.example.arc"),
                title: "Arc".into(),
                bundle_identifier: Some("com.example.arc".into()),
                path: "/Applications/Arc.app".into(),
            }],
            launched: Mutex::new(Vec::new()),
        });
        let index = Arc::new(StubIndex {
            configured: true,
            search_results,
            stats: AppIndexStats {
                discovered_count: 0,
                indexed_count: 1,
                skipped_count: 0,
            },
        });

        LauncherService::new(registry, platform, index)
    }

    #[test]
    fn aggregate_search_merges_command_and_app_results() {
        let launcher = build_launcher_service(vec!["app:macos:com.example.arc".into()]);

        let results = launcher.search("arc");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, SearchResultKind::Application);
        assert_eq!(results[0].subtitle.as_deref(), Some("com.example.arc"));
    }

    #[test]
    fn execute_routes_app_ids_to_platform_launcher() {
        let launcher = build_launcher_service(vec![]);

        let result = launcher
            .execute(&CommandId::from("app:macos:com.example.arc"), None)
            .unwrap();

        assert_eq!(result.output, "opened Arc");
    }

    #[test]
    fn execute_reindexes_apps() {
        let launcher = build_launcher_service(vec![]);

        let result = launcher
            .execute(&CommandId::from(APP_REINDEX_COMMAND_ID), None)
            .unwrap();

        assert_eq!(result.output, "reindexed 1 apps into Sonic (0 skipped)");
    }
}
