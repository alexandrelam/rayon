mod config;

pub use config::{load_config, LoadedConfig};

use rayon_db::{SearchIndexStats, TantivySearchIndex};
use rayon_platform::MacOsAppManager;
use rayon_types::{
    BookmarkDefinition, CommandDefinition, CommandExecutionRequest, CommandExecutionResult,
    CommandId, CommandInvocationResult, InstalledApp, InteractiveSessionMetadata,
    InteractiveSessionQueryRequest, InteractiveSessionResult, InteractiveSessionState,
    InteractiveSessionSubmitRequest, InteractiveSessionSubmitResult, ProcessMatch, SearchResult,
    SearchResultKind, SearchableItemDocument,
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub const APP_REINDEX_COMMAND_ID: &str = "apps.reindex";
const SEARCH_LIMIT: usize = 20;

pub trait CommandProvider: Send + Sync {
    fn commands(&self) -> Vec<CommandDefinition>;
    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError>;

    fn start_interactive_session(
        &self,
        _command_id: &CommandId,
    ) -> Result<Option<InteractiveSessionMetadata>, CommandError> {
        Ok(None)
    }

    fn search_interactive_session(
        &self,
        _session: &InteractiveSessionMetadata,
        _query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        Err(CommandError::ExecutionFailed(
            "interactive session search is not supported".into(),
        ))
    }

    fn submit_interactive_session(
        &self,
        _session: &InteractiveSessionMetadata,
        _query: &str,
        _item_id: &str,
    ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
        Err(CommandError::ExecutionFailed(
            "interactive session submit is not supported".into(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveSessionUpdate {
    pub results: Vec<InteractiveSessionResult>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractiveSessionSubmitOutcome {
    Updated(InteractiveSessionUpdate),
    Completed(CommandExecutionResult),
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
    InteractiveSessionNotFound(String),
    Platform(String),
    SearchBackend(String),
}

impl fmt::Display for LauncherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => write!(f, "{error}"),
            Self::AppNotFound(command_id) => write!(f, "unknown application id: {command_id}"),
            Self::InteractiveSessionNotFound(session_id) => {
                write!(f, "unknown interactive session: {session_id}")
            }
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
    starts_interactive_session: bool,
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

            let starts_interactive_session = provider
                .start_interactive_session(&definition.id)?
                .is_some();

            self.command_owners.insert(command_key, provider_index);
            self.commands.push(RegisteredCommand {
                definition,
                starts_interactive_session,
            });
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

    fn start_interactive_session(
        &self,
        command_id: &CommandId,
    ) -> Result<Option<InteractiveSessionOwner>, CommandError> {
        let provider_index = self
            .command_owners
            .get(command_id.as_str())
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(command_id.clone()))?;
        let provider = &self.providers[provider_index];
        let metadata = provider.start_interactive_session(command_id)?;
        Ok(metadata.map(|metadata| InteractiveSessionOwner {
            provider_index,
            metadata,
        }))
    }

    fn search_interactive_session(
        &self,
        provider_index: usize,
        session: &InteractiveSessionMetadata,
        query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        self.providers[provider_index].search_interactive_session(session, query)
    }

    fn submit_interactive_session(
        &self,
        provider_index: usize,
        session: &InteractiveSessionMetadata,
        query: &str,
        item_id: &str,
    ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
        self.providers[provider_index].submit_interactive_session(session, query, item_id)
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
                        starts_interactive_session: command.starts_interactive_session,
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
    fn search_processes(&self, query: &str) -> Result<Vec<ProcessMatch>, String>;
    fn terminate_process(&self, pid: u32) -> Result<(), String>;
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

    fn search_processes(&self, query: &str) -> Result<Vec<ProcessMatch>, String> {
        MacOsAppManager::search_processes(self, query)
    }

    fn terminate_process(&self, pid: u32) -> Result<(), String> {
        MacOsAppManager::terminate_process(self, pid)
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
                        starts_interactive_session: false,
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
                        starts_interactive_session: false,
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

fn read_interactive_sessions(
    sessions: &RwLock<HashMap<String, ActiveInteractiveSession>>,
) -> RwLockReadGuard<'_, HashMap<String, ActiveInteractiveSession>> {
    match sessions.read() {
        Ok(sessions) => sessions,
        Err(poisoned) => {
            eprintln!("interactive session lock poisoned while reading");
            poisoned.into_inner()
        }
    }
}

fn write_interactive_sessions(
    sessions: &RwLock<HashMap<String, ActiveInteractiveSession>>,
) -> RwLockWriteGuard<'_, HashMap<String, ActiveInteractiveSession>> {
    match sessions.write() {
        Ok(sessions) => sessions,
        Err(poisoned) => {
            eprintln!("interactive session lock poisoned while writing");
            poisoned.into_inner()
        }
    }
}

#[derive(Clone)]
struct InteractiveSessionOwner {
    provider_index: usize,
    metadata: InteractiveSessionMetadata,
}

#[derive(Clone)]
struct ActiveInteractiveSession {
    provider_index: usize,
    metadata: InteractiveSessionMetadata,
}

pub struct LauncherService {
    registry: CommandRegistry,
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
    app_catalog: RwLock<AppCatalog>,
    bookmark_catalog: BookmarkCatalog,
    interactive_sessions: RwLock<HashMap<String, ActiveInteractiveSession>>,
    next_session_id: AtomicU64,
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
            interactive_sessions: RwLock::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
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

    pub fn execute_command(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandInvocationResult, LauncherError> {
        if request.command_id.as_str() == APP_REINDEX_COMMAND_ID {
            let result = self.refresh_and_reindex()?;
            return Ok(CommandInvocationResult::Completed {
                output: result.output,
            });
        }

        if request.command_id.as_str().starts_with("app:macos:") {
            let result = self.launch_app(&request.command_id)?;
            return Ok(CommandInvocationResult::Completed {
                output: result.output,
            });
        }

        if self.bookmark_catalog.get(&request.command_id).is_some() {
            let result = self.open_bookmark(&request.command_id)?;
            return Ok(CommandInvocationResult::Completed {
                output: result.output,
            });
        }

        if let Some(session_owner) = self
            .registry
            .start_interactive_session(&request.command_id)?
        {
            let session_id = self.allocate_session_id();
            let metadata = InteractiveSessionMetadata {
                session_id: session_id.clone(),
                command_id: session_owner.metadata.command_id.clone(),
                title: session_owner.metadata.title,
                subtitle: session_owner.metadata.subtitle,
                input_placeholder: session_owner.metadata.input_placeholder,
            };

            write_interactive_sessions(&self.interactive_sessions).insert(
                session_id.clone(),
                ActiveInteractiveSession {
                    provider_index: session_owner.provider_index,
                    metadata: metadata.clone(),
                },
            );

            return Ok(CommandInvocationResult::StartedSession {
                session: InteractiveSessionState {
                    session_id,
                    command_id: metadata.command_id,
                    title: metadata.title,
                    subtitle: metadata.subtitle,
                    input_placeholder: metadata.input_placeholder,
                    query: String::new(),
                    is_loading: true,
                    results: Vec::new(),
                    message: None,
                },
            });
        }

        let result = self
            .registry
            .execute(request)
            .map_err(LauncherError::from)?;
        Ok(CommandInvocationResult::Completed {
            output: result.output,
        })
    }

    pub fn search_interactive_session(
        &self,
        request: &InteractiveSessionQueryRequest,
    ) -> Result<InteractiveSessionState, LauncherError> {
        let session = self.active_session(&request.session_id)?;
        let results = self.registry.search_interactive_session(
            session.provider_index,
            &session.metadata,
            &request.query,
        )?;

        Ok(InteractiveSessionState {
            session_id: session.metadata.session_id,
            command_id: session.metadata.command_id,
            title: session.metadata.title,
            subtitle: session.metadata.subtitle,
            input_placeholder: session.metadata.input_placeholder,
            query: request.query.clone(),
            is_loading: false,
            results,
            message: None,
        })
    }

    pub fn submit_interactive_session(
        &self,
        request: &InteractiveSessionSubmitRequest,
    ) -> Result<InteractiveSessionSubmitResult, LauncherError> {
        let session = self.active_session(&request.session_id)?;
        let outcome = self.registry.submit_interactive_session(
            session.provider_index,
            &session.metadata,
            &request.query,
            &request.item_id,
        )?;

        match outcome {
            InteractiveSessionSubmitOutcome::Updated(update) => {
                Ok(InteractiveSessionSubmitResult::UpdatedSession {
                    session: InteractiveSessionState {
                        session_id: session.metadata.session_id,
                        command_id: session.metadata.command_id,
                        title: session.metadata.title,
                        subtitle: session.metadata.subtitle,
                        input_placeholder: session.metadata.input_placeholder,
                        query: request.query.clone(),
                        is_loading: false,
                        results: update.results,
                        message: update.message,
                    },
                })
            }
            InteractiveSessionSubmitOutcome::Completed(result) => {
                write_interactive_sessions(&self.interactive_sessions).remove(&request.session_id);
                Ok(InteractiveSessionSubmitResult::Completed {
                    output: result.output,
                })
            }
        }
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

    fn active_session(&self, session_id: &str) -> Result<ActiveInteractiveSession, LauncherError> {
        read_interactive_sessions(&self.interactive_sessions)
            .get(session_id)
            .cloned()
            .ok_or_else(|| LauncherError::InteractiveSessionNotFound(session_id.to_string()))
    }

    fn allocate_session_id(&self) -> String {
        let next_id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        format!("session-{next_id}")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_types::{
        CommandArgumentDefinition, CommandArgumentType, InteractiveSessionMetadata,
        InteractiveSessionSubmitResult,
    };
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
                    arguments: vec![CommandArgumentDefinition {
                        id: String::from("message"),
                        label: String::from("Message"),
                        argument_type: CommandArgumentType::String,
                        required: true,
                        flag: None,
                        positional: Some(0),
                        default_value: None,
                    }],
                },
                CommandDefinition {
                    id: CommandId::from("kill"),
                    title: "Kill Process".into(),
                    subtitle: Some("Terminate a running process".into()),
                    owner_plugin_id: "builtin.kill".into(),
                    keywords: vec!["terminate".into()],
                    arguments: vec![],
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

        fn start_interactive_session(
            &self,
            command_id: &CommandId,
        ) -> Result<Option<InteractiveSessionMetadata>, CommandError> {
            if command_id.as_str() != "kill" {
                return Ok(None);
            }

            Ok(Some(InteractiveSessionMetadata {
                session_id: String::new(),
                command_id: command_id.clone(),
                title: "Kill Process".into(),
                subtitle: Some("Terminate a running process".into()),
                input_placeholder: "Search by process name or port".into(),
            }))
        }

        fn search_interactive_session(
            &self,
            _session: &InteractiveSessionMetadata,
            query: &str,
        ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
            Ok(vec![InteractiveSessionResult {
                id: format!("result:{query}"),
                title: "Arc".into(),
                subtitle: Some("PID 1234".into()),
            }])
        }

        fn submit_interactive_session(
            &self,
            _session: &InteractiveSessionMetadata,
            query: &str,
            item_id: &str,
        ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
            Ok(InteractiveSessionSubmitOutcome::Updated(
                InteractiveSessionUpdate {
                    results: vec![InteractiveSessionResult {
                        id: format!("refresh:{query}"),
                        title: "Preview".into(),
                        subtitle: Some("PID 99".into()),
                    }],
                    message: Some(format!("terminated {item_id}")),
                },
            ))
        }
    }

    struct StubPlatform {
        apps: Vec<InstalledApp>,
        launched: Mutex<Vec<String>>,
        opened_urls: Mutex<Vec<String>>,
        process_search_results: Mutex<Vec<ProcessMatch>>,
        terminated_pids: Mutex<Vec<u32>>,
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

        fn search_processes(&self, _query: &str) -> Result<Vec<ProcessMatch>, String> {
            Ok(self.process_search_results.lock().unwrap().clone())
        }

        fn terminate_process(&self, pid: u32) -> Result<(), String> {
            self.terminated_pids.lock().unwrap().push(pid);
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
            process_search_results: Mutex::new(Vec::new()),
            terminated_pids: Mutex::new(Vec::new()),
        });
        let index = Arc::new(StubSearchIndex {
            configured: true,
            search_results,
            stats: SearchIndexStats {
                discovered_count: 0,
                indexed_count: 5,
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
            .execute_command(&CommandExecutionRequest {
                command_id: CommandId::from("app:macos:com.example.arc"),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(
            result,
            CommandInvocationResult::Completed {
                output: "opened Arc".into()
            }
        );
    }

    #[test]
    fn execute_reindexes_shared_search_items() {
        let launcher = build_launcher_service(vec![]);

        let result = launcher
            .execute_command(&CommandExecutionRequest {
                command_id: CommandId::from(APP_REINDEX_COMMAND_ID),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(
            result,
            CommandInvocationResult::Completed {
                output: "reindexed 5 searchable items (0 skipped)".into()
            }
        );
    }

    #[test]
    fn execute_routes_bookmark_ids_to_platform_url_opener() {
        let launcher = build_launcher_service(vec![]);

        let result = launcher
            .execute_command(&CommandExecutionRequest {
                command_id: CommandId::from("bookmark:github"),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(
            result,
            CommandInvocationResult::Completed {
                output: "opened GitHub".into()
            }
        );
    }

    #[test]
    fn execute_starts_interactive_session_for_kill() {
        let launcher = build_launcher_service(vec![]);

        let result = launcher
            .execute_command(&CommandExecutionRequest {
                command_id: CommandId::from("kill"),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert!(matches!(
            result,
            CommandInvocationResult::StartedSession { .. }
        ));
        let CommandInvocationResult::StartedSession { session } = result else {
            unreachable!();
        };
        assert_eq!(session.command_id, CommandId::from("kill"));
        assert_eq!(session.title, "Kill Process");
        assert!(session.is_loading);
        assert!(session.results.is_empty());
    }

    #[test]
    fn interactive_session_search_and_submit_route_to_provider() {
        let launcher = build_launcher_service(vec![]);

        let session_result = launcher
            .execute_command(&CommandExecutionRequest {
                command_id: CommandId::from("kill"),
                arguments: HashMap::new(),
            })
            .unwrap();
        assert!(matches!(
            session_result,
            CommandInvocationResult::StartedSession { .. }
        ));
        let CommandInvocationResult::StartedSession { session } = session_result else {
            unreachable!();
        };

        let searched = launcher
            .search_interactive_session(&InteractiveSessionQueryRequest {
                session_id: session.session_id.clone(),
                query: "8080".into(),
            })
            .unwrap();
        assert_eq!(searched.query, "8080");
        assert!(!searched.is_loading);
        assert_eq!(searched.results[0].id, "result:8080");

        let submitted = launcher
            .submit_interactive_session(&InteractiveSessionSubmitRequest {
                session_id: session.session_id,
                query: "arc".into(),
                item_id: "1234".into(),
            })
            .unwrap();
        let session = match submitted {
            InteractiveSessionSubmitResult::UpdatedSession { session } => session,
            InteractiveSessionSubmitResult::Completed { .. } => unreachable!(),
        };
        assert_eq!(session.message, Some("terminated 1234".into()));
        assert_eq!(session.results[0].id, "refresh:arc");
    }

    #[test]
    fn completed_interactive_submit_removes_active_session() {
        struct CompletingProvider;

        impl CommandProvider for CompletingProvider {
            fn commands(&self) -> Vec<CommandDefinition> {
                vec![CommandDefinition {
                    id: CommandId::from("github.my-prs"),
                    title: "My Pull Requests".into(),
                    subtitle: Some("Open a pull request".into()),
                    owner_plugin_id: "builtin.github".into(),
                    keywords: vec!["github".into()],
                    arguments: vec![],
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
                if command_id.as_str() != "github.my-prs" {
                    return Ok(None);
                }

                Ok(Some(InteractiveSessionMetadata {
                    session_id: String::new(),
                    command_id: command_id.clone(),
                    title: "My Pull Requests".into(),
                    subtitle: Some("Open a pull request".into()),
                    input_placeholder: "Filter".into(),
                }))
            }

            fn search_interactive_session(
                &self,
                _session: &InteractiveSessionMetadata,
                _query: &str,
            ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
                Ok(vec![InteractiveSessionResult {
                    id: "https://github.com/example/repo/pull/1".into(),
                    title: "Fix bug".into(),
                    subtitle: Some("example/repo #1".into()),
                }])
            }

            fn submit_interactive_session(
                &self,
                _session: &InteractiveSessionMetadata,
                _query: &str,
                _item_id: &str,
            ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
                Ok(InteractiveSessionSubmitOutcome::Completed(
                    CommandExecutionResult {
                        output: "opened example/repo#1".into(),
                    },
                ))
            }
        }

        let mut registry = CommandRegistry::new();
        registry
            .register_provider(Arc::new(CompletingProvider))
            .unwrap();

        let platform = Arc::new(StubPlatform {
            apps: Vec::new(),
            launched: Mutex::new(Vec::new()),
            opened_urls: Mutex::new(Vec::new()),
            process_search_results: Mutex::new(Vec::new()),
            terminated_pids: Mutex::new(Vec::new()),
        });
        let index = Arc::new(StubSearchIndex {
            configured: true,
            search_results: Vec::new(),
            stats: SearchIndexStats {
                discovered_count: 0,
                indexed_count: 0,
                skipped_count: 0,
            },
            last_documents: Mutex::new(Vec::new()),
        });
        let launcher = LauncherService::new(registry, Vec::new(), platform, index);

        let session_result = launcher
            .execute_command(&CommandExecutionRequest {
                command_id: CommandId::from("github.my-prs"),
                arguments: HashMap::new(),
            })
            .unwrap();
        let session = match session_result {
            CommandInvocationResult::StartedSession { session } => session,
            CommandInvocationResult::Completed { .. } => unreachable!(),
        };

        let result = launcher
            .submit_interactive_session(&InteractiveSessionSubmitRequest {
                session_id: session.session_id.clone(),
                query: String::new(),
                item_id: "https://github.com/example/repo/pull/1".into(),
            })
            .unwrap();
        assert_eq!(
            result,
            InteractiveSessionSubmitResult::Completed {
                output: "opened example/repo#1".into()
            }
        );

        let error = launcher
            .search_interactive_session(&InteractiveSessionQueryRequest {
                session_id: session.session_id,
                query: String::new(),
            })
            .unwrap_err();
        assert_eq!(error.to_string(), "unknown interactive session: session-1");
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
            process_search_results: Mutex::new(Vec::new()),
            terminated_pids: Mutex::new(Vec::new()),
        });
        let index = Arc::new(StubSearchIndex {
            configured: true,
            search_results: Vec::new(),
            stats: SearchIndexStats {
                discovered_count: 0,
                indexed_count: 5,
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

        assert_eq!(documents.len(), 5);
        assert!(documents
            .iter()
            .any(|document| document.id == CommandId::from("hello")));
        assert!(documents
            .iter()
            .any(|document| document.id == CommandId::from("app:macos:com.example.arc")));
        assert!(documents
            .iter()
            .any(|document| document.id == CommandId::from("bookmark:github")));
        assert!(documents
            .iter()
            .any(|document| document.id == CommandId::from("kill")));
    }
}
