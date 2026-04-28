use super::{adapters, clipboard, launcher, paths};
use rayon_core::{AppPlatform, LauncherService, SearchIndex, APP_REINDEX_COMMAND_ID};
use rayon_features::{ClipboardHistoryService, ThemeSettingsStore};
use rayon_types::{
    BrowserTab, CommandExecutionRequest, CommandExecutionResult, CommandInvocationResult,
    InteractiveSessionQueryRequest, InteractiveSessionState, InteractiveSessionSubmitRequest,
    InteractiveSessionSubmitResult, OpenWindow, SearchResult, SearchResultKind,
    SearchableItemDocument,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tauri::AppHandle;

const LEADING_SPACE_SEARCH_LIMIT: usize = 20;

pub struct AppState {
    launcher: RwLock<LauncherService>,
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
    clipboard_history: Arc<ClipboardHistoryService>,
    theme_settings: Arc<ThemeSettingsStore>,
    leading_space_search_cache: Mutex<LeadingSpaceSearchCache>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app_index = Arc::new(adapters::DesktopSearchIndex::open_or_create(
            paths::app_search_index_path(app)?,
        )?);
        let platform = Arc::new(adapters::DesktopPlatform::new());
        let clipboard_access = Arc::new(clipboard::MacOsClipboardAccess);
        let clipboard_history = Arc::new(ClipboardHistoryService::new(
            clipboard_access.clone(),
            paths::app_clipboard_history_path(app)?,
        )?);
        let theme_settings = Arc::new(ThemeSettingsStore::new(paths::app_theme_settings_path(
            app,
        )?));
        let launcher = launcher::build_launcher(
            platform.clone(),
            app_index.clone(),
            clipboard_history.clone(),
            theme_settings.clone(),
        )?;
        clipboard::spawn_clipboard_watcher(clipboard_history.clone(), clipboard_access);

        Ok(Self {
            launcher: RwLock::new(launcher),
            platform,
            search_index: app_index,
            clipboard_history,
            theme_settings,
            leading_space_search_cache: Mutex::new(LeadingSpaceSearchCache::new()?),
        })
    }

    pub fn reload(&self) -> Result<CommandExecutionResult, String> {
        launcher::reload_launcher(
            &self.launcher,
            self.platform.clone(),
            self.search_index.clone(),
            self.clipboard_history.clone(),
            self.theme_settings.clone(),
        )
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        self.read_launcher().search(query)
    }

    pub fn search_browser_tabs(&self, query: &str, refresh: bool) -> Vec<SearchResult> {
        match self.leading_space_search_cache.lock() {
            Ok(mut cache) => cache.search(self.platform.as_ref(), query, refresh),
            Err(poisoned) => {
                eprintln!("leading-space search cache lock poisoned");
                poisoned
                    .into_inner()
                    .search(self.platform.as_ref(), query, refresh)
            }
        }
    }

    pub fn execute_command(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandInvocationResult, String> {
        if request.command_id.as_str() == APP_REINDEX_COMMAND_ID {
            return self
                .reload()
                .map(|result| CommandInvocationResult::Completed {
                    output: result.output,
                });
        }

        self.read_launcher()
            .execute_command(request)
            .map_err(|error| error.to_string())
    }

    pub fn search_interactive_session(
        &self,
        request: &InteractiveSessionQueryRequest,
    ) -> Result<InteractiveSessionState, String> {
        self.read_launcher()
            .search_interactive_session(request)
            .map_err(|error| error.to_string())
    }

    pub fn submit_interactive_session(
        &self,
        request: &InteractiveSessionSubmitRequest,
    ) -> Result<InteractiveSessionSubmitResult, String> {
        self.read_launcher()
            .submit_interactive_session(request)
            .map_err(|error| error.to_string())
    }

    pub fn read_launcher(&self) -> RwLockReadGuard<'_, LauncherService> {
        match self.launcher.read() {
            Ok(launcher) => launcher,
            Err(poisoned) => {
                eprintln!("launcher lock poisoned while reading");
                poisoned.into_inner()
            }
        }
    }

    pub fn theme_preference(&self) -> rayon_types::ThemePreference {
        match self.theme_settings.load() {
            Ok(theme) => theme,
            Err(error) => {
                eprintln!("failed to load theme settings: {error}");
                rayon_types::ThemePreference::System
            }
        }
    }
}

struct LeadingSpaceSearchCache {
    items_by_id: HashMap<String, LeadingSpaceItem>,
    ordered_item_ids: Vec<String>,
    search_index: rayon_db::TantivySearchIndex,
    initialized: bool,
}

impl LeadingSpaceSearchCache {
    fn new() -> Result<Self, String> {
        Ok(Self {
            items_by_id: HashMap::new(),
            ordered_item_ids: Vec::new(),
            search_index: rayon_db::TantivySearchIndex::create_in_memory()
                .map_err(|error| error.to_string())?,
            initialized: false,
        })
    }

    fn search(
        &mut self,
        platform: &dyn AppPlatform,
        query: &str,
        refresh: bool,
    ) -> Vec<SearchResult> {
        if refresh || !self.initialized {
            if let Err(error) = self.refresh(platform) {
                eprintln!("leading-space refresh failed: {error}");
                return Vec::new();
            }
        }

        let normalized_query = query.trim();
        if normalized_query.is_empty() {
            return self
                .ordered_item_ids
                .iter()
                .take(LEADING_SPACE_SEARCH_LIMIT)
                .filter_map(|item_id| self.items_by_id.get(item_id))
                .map(LeadingSpaceItem::search_result)
                .collect();
        }

        let item_ids = match self
            .search_index
            .search_item_ids(normalized_query, LEADING_SPACE_SEARCH_LIMIT)
        {
            Ok(item_ids) => item_ids,
            Err(error) => {
                eprintln!("leading-space index search failed: {error}");
                return Vec::new();
            }
        };

        let mut matches = item_ids
            .into_iter()
            .enumerate()
            .filter_map(|(index, item_id)| {
                self.items_by_id
                    .get(&item_id)
                    .map(|item| (leading_space_sort_key(item, index), item.search_result()))
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.0.cmp(&right.0));
        matches.into_iter().map(|(_, result)| result).collect()
    }

    fn refresh(&mut self, platform: &dyn AppPlatform) -> Result<(), String> {
        let tabs = platform.search_browser_tabs("")?;
        let windows = platform.list_open_windows()?;
        let ordered_item_ids = windows
            .iter()
            .map(OpenWindow::command_id)
            .chain(tabs.iter().map(BrowserTab::command_id))
            .map(|id| id.to_string())
            .collect::<Vec<_>>();

        let mut documents = windows
            .iter()
            .map(LeadingSpaceItem::window_search_document)
            .collect::<Vec<_>>();
        documents.extend(
            tabs.iter()
                .map(LeadingSpaceItem::tab_search_document)
                .collect::<Vec<_>>(),
        );

        self.search_index
            .replace_items(&documents)
            .map_err(|error| error.to_string())?;

        self.items_by_id = windows
            .into_iter()
            .map(|window| {
                let item = LeadingSpaceItem::OpenWindow(window);
                (item.id().to_string(), item)
            })
            .chain(tabs.into_iter().map(|tab| {
                let item = LeadingSpaceItem::BrowserTab(tab);
                (item.id().to_string(), item)
            }))
            .collect();

        self.ordered_item_ids = ordered_item_ids;
        self.initialized = true;
        Ok(())
    }
}

#[derive(Clone)]
enum LeadingSpaceItem {
    BrowserTab(BrowserTab),
    OpenWindow(OpenWindow),
}

impl LeadingSpaceItem {
    fn id(&self) -> rayon_types::CommandId {
        match self {
            Self::BrowserTab(tab) => tab.command_id(),
            Self::OpenWindow(window) => window.command_id(),
        }
    }

    fn search_result(&self) -> SearchResult {
        match self {
            Self::BrowserTab(tab) => SearchResult {
                id: tab.command_id(),
                title: tab.title.clone(),
                subtitle: Some(tab.subtitle()),
                icon_path: None,
                kind: SearchResultKind::BrowserTab,
                owner_plugin_id: None,
                keywords: Vec::new(),
                starts_interactive_session: false,
                close_launcher_on_success: true,
                input_mode: rayon_types::CommandInputMode::Structured,
                arguments: Vec::new(),
            },
            Self::OpenWindow(window) => SearchResult {
                id: window.command_id(),
                title: window.display_title(),
                subtitle: Some(window.subtitle()),
                icon_path: None,
                kind: SearchResultKind::OpenWindow,
                owner_plugin_id: None,
                keywords: Vec::new(),
                starts_interactive_session: false,
                close_launcher_on_success: true,
                input_mode: rayon_types::CommandInputMode::Structured,
                arguments: Vec::new(),
            },
        }
    }

    fn tab_search_document(tab: &BrowserTab) -> SearchableItemDocument {
        SearchableItemDocument {
            id: tab.command_id(),
            kind: SearchResultKind::BrowserTab,
            title: tab.title.clone(),
            subtitle: Some(tab.subtitle()),
            owner_plugin_id: None,
            search_text: tab.search_text(),
        }
    }

    fn window_search_document(window: &OpenWindow) -> SearchableItemDocument {
        SearchableItemDocument {
            id: window.command_id(),
            kind: SearchResultKind::OpenWindow,
            title: window.display_title(),
            subtitle: Some(window.subtitle()),
            owner_plugin_id: None,
            search_text: window.search_text(),
        }
    }
}

fn leading_space_sort_key(item: &LeadingSpaceItem, original_index: usize) -> (u8, u8, usize) {
    match item {
        LeadingSpaceItem::OpenWindow(window) => {
            (0, if window.is_frontmost { 0 } else { 1 }, original_index)
        }
        LeadingSpaceItem::BrowserTab(tab) => {
            (1, if tab.is_active() { 0 } else { 1 }, original_index)
        }
    }
}

pub fn write_launcher(launcher: &RwLock<LauncherService>) -> RwLockWriteGuard<'_, LauncherService> {
    match launcher.write() {
        Ok(launcher) => launcher,
        Err(poisoned) => {
            eprintln!("launcher lock poisoned while writing");
            poisoned.into_inner()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_types::{
        BrowserTabTarget, CommandId, InstalledApp, OpenWindow, OpenWindowTarget, ProcessMatch,
    };
    use std::sync::Mutex;

    struct StubPlatform {
        browser_tabs: Mutex<Vec<BrowserTab>>,
        open_windows: Mutex<Vec<OpenWindow>>,
        search_calls: Mutex<usize>,
    }

    impl AppPlatform for StubPlatform {
        fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
            Ok(Vec::new())
        }

        fn launch_app(&self, _app: &InstalledApp) -> Result<(), String> {
            Ok(())
        }

        fn open_url(&self, _url: &str) -> Result<(), String> {
            Ok(())
        }

        fn copy_image_to_clipboard(&self, _image_path: &std::path::Path) -> Result<(), String> {
            Ok(())
        }

        fn search_browser_tabs(&self, _query: &str) -> Result<Vec<BrowserTab>, String> {
            *self.search_calls.lock().unwrap() += 1;
            Ok(self.browser_tabs.lock().unwrap().clone())
        }

        fn focus_browser_tab(&self, _target: &BrowserTabTarget) -> Result<(), String> {
            Ok(())
        }

        fn list_open_windows(&self) -> Result<Vec<OpenWindow>, String> {
            *self.search_calls.lock().unwrap() += 1;
            Ok(self.open_windows.lock().unwrap().clone())
        }

        fn focus_open_window(&self, _target: &OpenWindowTarget) -> Result<(), String> {
            Ok(())
        }

        fn search_processes(&self, _query: &str) -> Result<Vec<ProcessMatch>, String> {
            Ok(Vec::new())
        }

        fn terminate_process(&self, _pid: u32) -> Result<(), String> {
            Ok(())
        }
    }

    fn sample_tab(title: &str, url: &str, window_index: u32, tab_index: u32) -> BrowserTab {
        BrowserTab {
            browser: "chrome".into(),
            window_id: format!("window-{window_index}"),
            window_index,
            active_tab_index: tab_index,
            tab_index,
            title: title.into(),
            url: url.into(),
        }
    }

    fn sample_window(title: &str, application: &str, pid: i32, frontmost: bool) -> OpenWindow {
        OpenWindow {
            application: application.into(),
            pid,
            window_number: pid * 100,
            bounds_x: pid * 10,
            bounds_y: pid * 10,
            bounds_width: 1440,
            bounds_height: 900,
            title: title.into(),
            is_frontmost: frontmost,
        }
    }

    #[test]
    fn leading_space_search_cache_refreshes_only_on_request() {
        let platform = StubPlatform {
            browser_tabs: Mutex::new(vec![sample_tab(
                "Issue 15",
                "https://github.com/alexandrelam/rayon/issues/15",
                1,
                1,
            )]),
            open_windows: Mutex::new(vec![sample_window("Rayon", "Arc", 101, true)]),
            search_calls: Mutex::new(0),
        };
        let mut cache = LeadingSpaceSearchCache::new().unwrap();

        let first_results = cache.search(&platform, "issue", true);
        let second_results = cache.search(&platform, "rayon", false);

        assert_eq!(first_results.len(), 1);
        assert_eq!(second_results.len(), 2);
        assert_eq!(*platform.search_calls.lock().unwrap(), 2);
    }

    #[test]
    fn leading_space_search_cache_replaces_stale_items_on_refresh() {
        let platform = StubPlatform {
            browser_tabs: Mutex::new(vec![sample_tab("Old Tab", "https://old.example", 1, 1)]),
            open_windows: Mutex::new(vec![sample_window("Old Window", "Arc", 202, true)]),
            search_calls: Mutex::new(0),
        };
        let mut cache = LeadingSpaceSearchCache::new().unwrap();

        let old_results = cache.search(&platform, "old", true);
        assert_eq!(old_results.len(), 2);

        *platform.browser_tabs.lock().unwrap() =
            vec![sample_tab("Fresh Tab", "https://fresh.example/path", 1, 1)];
        *platform.open_windows.lock().unwrap() =
            vec![sample_window("Fresh Window", "Finder", 303, true)];

        let stale_results = cache.search(&platform, "fresh", false);
        assert!(stale_results.is_empty());

        let fresh_results = cache.search(&platform, "fresh", true);
        assert_eq!(fresh_results.len(), 2);
        assert_eq!(*platform.search_calls.lock().unwrap(), 4);
    }

    #[test]
    fn leading_space_search_cache_returns_windows_then_tabs_for_blank_query() {
        let platform = StubPlatform {
            browser_tabs: Mutex::new(vec![
                sample_tab("Current", "https://current.example", 1, 1),
                sample_tab("Other", "https://other.example", 2, 1),
            ]),
            open_windows: Mutex::new(vec![
                sample_window("Project", "Arc", 404, true),
                sample_window("Notes", "Obsidian", 505, false),
            ]),
            search_calls: Mutex::new(0),
        };
        let mut cache = LeadingSpaceSearchCache::new().unwrap();

        let results = cache.search(&platform, "", true);

        assert_eq!(results.len(), 4);
        assert_eq!(results[0].kind, SearchResultKind::OpenWindow);
        assert_eq!(
            results[0].id,
            CommandId::from("open-window:404:40400:4040:4040:1440:900")
        );
        assert_eq!(results[2].kind, SearchResultKind::BrowserTab);
        assert_eq!(
            results[2].id,
            CommandId::from("browser-tab:chrome:window-1:1")
        );
    }

    #[test]
    fn leading_space_search_prioritizes_windows_over_tabs_for_matching_queries() {
        let platform = StubPlatform {
            browser_tabs: Mutex::new(vec![sample_tab(
                "Project Docs",
                "https://example.com/docs",
                1,
                1,
            )]),
            open_windows: Mutex::new(vec![sample_window("Project Board", "Linear", 606, true)]),
            search_calls: Mutex::new(0),
        };
        let mut cache = LeadingSpaceSearchCache::new().unwrap();

        let results = cache.search(&platform, "project", true);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].kind, SearchResultKind::OpenWindow);
        assert_eq!(results[1].kind, SearchResultKind::BrowserTab);
    }
}
