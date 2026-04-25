mod config;

pub use config::{load_config, LoadedConfig};

use rayon_db::{SearchIndexStats, TantivySearchIndex};
use rayon_platform::MacOsAppManager;
use rayon_types::{
    BookmarkDefinition, CommandDefinition, CommandExecutionRequest, CommandExecutionResult,
    CommandId, InstalledApp, SearchResult, SearchResultKind, SearchableItemDocument,
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub const APP_REINDEX_COMMAND_ID: &str = "apps.reindex";
const SEARCH_LIMIT: usize = 20;

pub trait CommandProvider: Send + Sync {
    fn commands(&self) -> Vec<CommandDefinition>;
    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    DuplicateCommandId(CommandId),
    UnknownCommand(CommandId),
    InvalidArguments(String),
    ExecutionFailed(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCommandId(command_id) => {
                write!(f, "duplicate command id registered: {command_id}")
            }
            Self::UnknownCommand(command_id) => write!(f, "unknown command id: {command_id}"),
            Self::InvalidArguments(error) => write!(f, "{error}"),
            Self::ExecutionFailed(error) => write!(f, "{error}"),
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

#[derive(Default)]
pub struct CommandRegistry {
    providers: Vec<Arc<dyn CommandProvider>>,
    commands: Vec<RegisteredCommand>,
    command_owners: HashMap<String, usize>,
}

#[derive(Clone)]
struct RegisteredCommand {
    definition: CommandDefinition,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
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

    pub fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        let provider_index = self
            .command_owners
            .get(request.command_id.as_str())
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(request.command_id.clone()))?;

        self.providers[provider_index].execute(request)
    }

    pub fn search_results_by_id(&self) -> HashMap<String, SearchResult> {
        self.commands
            .iter()
            .map(|command| {
                (
                    command.definition.id.to_string(),
                    SearchResult {
                        id: command.definition.id.clone(),
                        title: command.definition.title.clone(),
                        subtitle: command.definition.subtitle.clone(),
                        icon_path: None,
                        kind: SearchResultKind::Command,
                        owner_plugin_id: Some(command.definition.owner_plugin_id.clone()),
                        arguments: command.definition.arguments.clone(),
                    },
                )
            })
            .collect()
    }

    pub fn searchable_documents(&self) -> Vec<SearchableItemDocument> {
        self.commands
            .iter()
            .map(|command| SearchableItemDocument {
                id: command.definition.id.clone(),
                kind: SearchResultKind::Command,
                title: command.definition.title.clone(),
                subtitle: command.definition.subtitle.clone(),
                owner_plugin_id: Some(command.definition.owner_plugin_id.clone()),
                search_text: command_search_text(&command.definition),
            })
            .collect()
    }
}

fn command_search_text(definition: &CommandDefinition) -> String {
    let mut parts = vec![
        definition.id.to_string(),
        definition.title.clone(),
        definition.owner_plugin_id.clone(),
    ];
    if let Some(subtitle) = &definition.subtitle {
        parts.push(subtitle.clone());
    }
    parts.extend(definition.keywords.clone());
    parts.extend(
        definition
            .arguments
            .iter()
            .map(|argument| argument.label.clone()),
    );
    parts.join(" ")
}

pub trait AppPlatform: Send + Sync {
    fn discover_apps(&self) -> Result<Vec<InstalledApp>, String>;
    fn launch_app(&self, app: &InstalledApp) -> Result<(), String>;
    fn open_url(&self, url: &str) -> Result<(), String>;
}

pub trait SearchIndex: Send + Sync {
    fn is_configured(&self) -> bool;
    fn search_item_ids(&self, query: &str, limit: usize) -> Result<Vec<String>, String>;
    fn replace_items(&self, items: &[SearchableItemDocument]) -> Result<SearchIndexStats, String>;
}

impl AppPlatform for MacOsAppManager {
    fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
        MacOsAppManager::discover_apps(self)
    }

    fn launch_app(&self, app: &InstalledApp) -> Result<(), String> {
        MacOsAppManager::launch_app(self, app)
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        MacOsAppManager::open_url(self, url)
    }
}

impl SearchIndex for TantivySearchIndex {
    fn is_configured(&self) -> bool {
        TantivySearchIndex::is_configured(self)
    }

    fn search_item_ids(&self, query: &str, limit: usize) -> Result<Vec<String>, String> {
        TantivySearchIndex::search_item_ids(self, query, limit).map_err(|error| error.to_string())
    }

    fn replace_items(&self, items: &[SearchableItemDocument]) -> Result<SearchIndexStats, String> {
        TantivySearchIndex::replace_items(self, items).map_err(|error| error.to_string())
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

    fn searchable_documents(&self) -> Vec<SearchableItemDocument> {
        self.by_id
            .values()
            .map(|app| SearchableItemDocument {
                id: app.id.clone(),
                kind: SearchResultKind::Application,
                title: app.title.clone(),
                subtitle: Some(app.subtitle()),
                owner_plugin_id: None,
                search_text: app.search_text(),
            })
            .collect()
    }

    fn search_results_by_id(&self) -> HashMap<String, SearchResult> {
        self.by_id
            .values()
            .map(|app| {
                (
                    app.id.to_string(),
                    SearchResult {
                        id: app.id.clone(),
                        title: app.title.clone(),
                        subtitle: Some(app.subtitle()),
                        icon_path: None,
                        kind: SearchResultKind::Application,
                        owner_plugin_id: None,
                        arguments: Vec::new(),
                    },
                )
            })
            .collect()
    }
}

#[derive(Default)]
struct BookmarkCatalog {
    by_id: HashMap<String, BookmarkDefinition>,
}

impl BookmarkCatalog {
    fn from_bookmarks(bookmarks: Vec<BookmarkDefinition>) -> Self {
        let mut by_id = HashMap::new();
        for bookmark in bookmarks {
            by_id.insert(bookmark.id.to_string(), bookmark);
        }

        Self { by_id }
    }

    fn get(&self, bookmark_id: &CommandId) -> Option<&BookmarkDefinition> {
        self.by_id.get(bookmark_id.as_str())
    }

    fn search_results_by_id(&self) -> HashMap<String, SearchResult> {
        self.by_id
            .values()
            .map(|bookmark| {
                (
                    bookmark.id.to_string(),
                    SearchResult {
                        id: bookmark.id.clone(),
                        title: bookmark.title.clone(),
                        subtitle: bookmark.subtitle.clone(),
                        icon_path: None,
                        kind: SearchResultKind::Bookmark,
                        owner_plugin_id: Some(bookmark.owner_plugin_id.clone()),
                        arguments: Vec::new(),
                    },
                )
            })
            .collect()
    }

    fn searchable_documents(&self) -> Vec<SearchableItemDocument> {
        self.by_id
            .values()
            .map(|bookmark| SearchableItemDocument {
                id: bookmark.id.clone(),
                kind: SearchResultKind::Bookmark,
                title: bookmark.title.clone(),
                subtitle: bookmark.subtitle.clone(),
                owner_plugin_id: Some(bookmark.owner_plugin_id.clone()),
                search_text: bookmark_search_text(bookmark),
            })
            .collect()
    }
}

fn bookmark_search_text(definition: &BookmarkDefinition) -> String {
    let mut parts = vec![
        definition.id.to_string(),
        definition.title.clone(),
        definition.owner_plugin_id.clone(),
        definition.url.clone(),
    ];
    if let Some(subtitle) = &definition.subtitle {
        parts.push(subtitle.clone());
    }
    parts.extend(definition.keywords.clone());
    parts.join(" ")
}

fn read_app_catalog(app_catalog: &RwLock<AppCatalog>) -> RwLockReadGuard<'_, AppCatalog> {
    match app_catalog.read() {
        Ok(app_catalog) => app_catalog,
        Err(poisoned) => {
            eprintln!("app catalog lock poisoned while reading");
            poisoned.into_inner()
        }
    }
}

fn write_app_catalog(app_catalog: &RwLock<AppCatalog>) -> RwLockWriteGuard<'_, AppCatalog> {
    match app_catalog.write() {
        Ok(app_catalog) => app_catalog,
        Err(poisoned) => {
            eprintln!("app catalog lock poisoned while writing");
            poisoned.into_inner()
        }
    }
}

pub struct LauncherService {
    registry: CommandRegistry,
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
    app_catalog: RwLock<AppCatalog>,
    bookmark_catalog: BookmarkCatalog,
}

impl LauncherService {
    pub fn new(
        registry: CommandRegistry,
        bookmarks: Vec<BookmarkDefinition>,
        platform: Arc<dyn AppPlatform>,
        search_index: Arc<dyn SearchIndex>,
    ) -> Self {
        let app_catalog = match platform.discover_apps() {
            Ok(apps) => AppCatalog::from_apps(apps),
            Err(error) => {
                eprintln!("failed to discover apps on startup: {error}");
                AppCatalog::default()
            }
        };

        let service = Self {
            registry,
            platform,
            search_index,
            app_catalog: RwLock::new(app_catalog),
            bookmark_catalog: BookmarkCatalog::from_bookmarks(bookmarks),
        };

        if let Err(error) = service.reindex_search() {
            eprintln!("failed to build search index on startup: {error}");
        }

        service
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let item_ids = match self.search_index.search_item_ids(query, SEARCH_LIMIT) {
            Ok(item_ids) => item_ids,
            Err(error) => {
                eprintln!("search failed: {error}");
                Vec::new()
            }
        };

        let mut search_results = self.registry.search_results_by_id();
        let app_results = read_app_catalog(&self.app_catalog).search_results_by_id();
        search_results.extend(app_results);
        search_results.extend(self.bookmark_catalog.search_results_by_id());

        item_ids
            .into_iter()
            .filter_map(|item_id| search_results.get(&item_id).cloned())
            .collect()
    }

    pub fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, LauncherError> {
        if request.command_id.as_str() == APP_REINDEX_COMMAND_ID {
            return self.refresh_and_reindex();
        }

        if request.command_id.as_str().starts_with("app:macos:") {
            return self.launch_app(&request.command_id);
        }

        if self.bookmark_catalog.get(&request.command_id).is_some() {
            return self.open_bookmark(&request.command_id);
        }

        self.registry.execute(request).map_err(LauncherError::from)
    }

    pub fn search_enabled(&self) -> bool {
        self.search_index.is_configured()
    }

    fn refresh_and_reindex(&self) -> Result<CommandExecutionResult, LauncherError> {
        let apps = self
            .platform
            .discover_apps()
            .map_err(LauncherError::Platform)?;
        {
            let mut app_catalog = write_app_catalog(&self.app_catalog);
            *app_catalog = AppCatalog::from_apps(apps);
        }

        let stats = self
            .reindex_search()
            .map_err(LauncherError::SearchBackend)?;
        Ok(CommandExecutionResult {
            output: format!(
                "reindexed {} searchable items ({} skipped)",
                stats.indexed_count, stats.skipped_count
            ),
        })
    }

    fn reindex_search(&self) -> Result<SearchIndexStats, String> {
        let mut documents = self.registry.searchable_documents();
        let app_documents = read_app_catalog(&self.app_catalog).searchable_documents();
        documents.extend(app_documents);
        documents.extend(self.bookmark_catalog.searchable_documents());
        self.search_index.replace_items(&documents)
    }

    fn launch_app(&self, command_id: &CommandId) -> Result<CommandExecutionResult, LauncherError> {
        let app = {
            let app_catalog = read_app_catalog(&self.app_catalog);
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

    fn open_bookmark(
        &self,
        bookmark_id: &CommandId,
    ) -> Result<CommandExecutionResult, LauncherError> {
        let bookmark = self
            .bookmark_catalog
            .get(bookmark_id)
            .cloned()
            .ok_or_else(|| LauncherError::AppNotFound(bookmark_id.clone()))?;

        self.platform
            .open_url(&bookmark.url)
            .map_err(LauncherError::Platform)?;

        Ok(CommandExecutionResult {
            output: format!("opened {}", bookmark.title),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_types::{CommandArgumentType, CommandExecutionRequest};
    use std::sync::Mutex;

    struct TestProvider;

    impl CommandProvider for TestProvider {
        fn commands(&self) -> Vec<CommandDefinition> {
            vec![
                CommandDefinition {
                    id: CommandId::from("hello"),
                    title: "Hello".into(),
                    subtitle: Some("Greet".into()),
                    owner_plugin_id: "builtin.hello".into(),
                    keywords: vec![String::from("greeting")],
                    arguments: vec![],
                },
                CommandDefinition {
                    id: CommandId::from("echo"),
                    title: "Echo".into(),
                    subtitle: None,
                    owner_plugin_id: "builtin.echo".into(),
                    keywords: vec![],
                    arguments: vec![rayon_types::CommandArgumentDefinition {
                        id: String::from("message"),
                        label: String::from("Message"),
                        argument_type: CommandArgumentType::String,
                        required: true,
                        flag: None,
                        positional: Some(0),
                        default_value: None,
                    }],
                },
            ]
        }

        fn execute(
            &self,
            request: &CommandExecutionRequest,
        ) -> Result<CommandExecutionResult, CommandError> {
            Ok(CommandExecutionResult {
                output: format!("ran:{}", request.command_id),
            })
        }
    }

    struct StubPlatform {
        apps: Vec<InstalledApp>,
        launched: Mutex<Vec<String>>,
        opened_urls: Mutex<Vec<String>>,
    }

    impl AppPlatform for StubPlatform {
        fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
            Ok(self.apps.clone())
        }

        fn launch_app(&self, app: &InstalledApp) -> Result<(), String> {
            self.launched.lock().unwrap().push(app.id.to_string());
            Ok(())
        }

        fn open_url(&self, url: &str) -> Result<(), String> {
            self.opened_urls.lock().unwrap().push(url.to_string());
            Ok(())
        }
    }

    struct StubSearchIndex {
        configured: bool,
        search_results: Vec<String>,
        stats: SearchIndexStats,
        last_documents: Mutex<Vec<SearchableItemDocument>>,
    }

    impl SearchIndex for StubSearchIndex {
        fn is_configured(&self) -> bool {
            self.configured
        }

        fn search_item_ids(&self, _query: &str, _limit: usize) -> Result<Vec<String>, String> {
            Ok(self.search_results.clone())
        }

        fn replace_items(
            &self,
            items: &[SearchableItemDocument],
        ) -> Result<SearchIndexStats, String> {
            *self.last_documents.lock().unwrap() = items.to_vec();
            Ok(SearchIndexStats {
                discovered_count: items.len(),
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
            opened_urls: Mutex::new(Vec::new()),
        });
        let index = Arc::new(StubSearchIndex {
            configured: true,
            search_results,
            stats: SearchIndexStats {
                discovered_count: 0,
                indexed_count: 4,
                skipped_count: 0,
            },
            last_documents: Mutex::new(Vec::new()),
        });

        LauncherService::new(
            registry,
            vec![BookmarkDefinition {
                id: CommandId::from("bookmark:github"),
                title: "GitHub".into(),
                subtitle: Some("Code hosting".into()),
                owner_plugin_id: "user.links".into(),
                url: "https://github.com".into(),
                keywords: vec!["git".into(), "repos".into()],
            }],
            platform,
            index,
        )
    }

    #[test]
    fn registry_exposes_argument_metadata() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let results = registry.search_results_by_id();
        assert_eq!(results["echo"].arguments.len(), 1);
    }

    #[test]
    fn executes_command_through_provider() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let result = registry
            .execute(&CommandExecutionRequest {
                command_id: CommandId::from("hello"),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(result.output, "ran:hello");
    }

    #[test]
    fn returns_error_for_unknown_command() {
        let registry = CommandRegistry::new();

        let error = registry
            .execute(&CommandExecutionRequest {
                command_id: CommandId::from("missing"),
                arguments: HashMap::new(),
            })
            .unwrap_err();

        assert_eq!(
            error,
            CommandError::UnknownCommand(CommandId::from("missing"))
        );
    }

    #[test]
    fn aggregate_search_uses_shared_index_ids() {
        let launcher = build_launcher_service(vec![
            String::from("hello"),
            String::from("app:macos:com.example.arc"),
            String::from("bookmark:github"),
        ]);

        let results = launcher.search("arc");

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].kind, SearchResultKind::Command);
        assert_eq!(results[1].kind, SearchResultKind::Application);
        assert_eq!(results[2].kind, SearchResultKind::Bookmark);
    }

    #[test]
    fn execute_routes_app_ids_to_platform_launcher() {
        let launcher = build_launcher_service(vec![]);

        let result = launcher
            .execute(&CommandExecutionRequest {
                command_id: CommandId::from("app:macos:com.example.arc"),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(result.output, "opened Arc");
    }

    #[test]
    fn execute_reindexes_shared_search_items() {
        let launcher = build_launcher_service(vec![]);

        let result = launcher
            .execute(&CommandExecutionRequest {
                command_id: CommandId::from(APP_REINDEX_COMMAND_ID),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(result.output, "reindexed 4 searchable items (0 skipped)");
    }

    #[test]
    fn execute_routes_bookmark_ids_to_platform_url_opener() {
        let launcher = build_launcher_service(vec![]);

        let result = launcher
            .execute(&CommandExecutionRequest {
                command_id: CommandId::from("bookmark:github"),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(result.output, "opened GitHub");
    }

    #[test]
    fn startup_reindexes_commands_and_apps() {
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
            opened_urls: Mutex::new(Vec::new()),
        });
        let index = Arc::new(StubSearchIndex {
            configured: true,
            search_results: Vec::new(),
            stats: SearchIndexStats {
                discovered_count: 0,
                indexed_count: 4,
                skipped_count: 0,
            },
            last_documents: Mutex::new(Vec::new()),
        });

        let _launcher = LauncherService::new(
            registry,
            vec![BookmarkDefinition {
                id: CommandId::from("bookmark:github"),
                title: "GitHub".into(),
                subtitle: Some("Code hosting".into()),
                owner_plugin_id: "user.links".into(),
                url: "https://github.com".into(),
                keywords: vec!["git".into()],
            }],
            platform,
            index.clone(),
        );
        let documents = index.last_documents.lock().unwrap();

        assert_eq!(documents.len(), 4);
        assert!(documents
            .iter()
            .any(|document| document.id == CommandId::from("hello")));
        assert!(documents
            .iter()
            .any(|document| document.id == CommandId::from("app:macos:com.example.arc")));
        assert!(documents
            .iter()
            .any(|document| document.id == CommandId::from("bookmark:github")));
    }
}
