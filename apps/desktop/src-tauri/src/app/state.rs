use super::{launcher, paths};
use rayon_core::{AppPlatform, LauncherService, SearchIndex};
use rayon_db::TantivySearchIndex;
use rayon_features::ThemeSettingsStore;
use rayon_platform::MacOsAppManager;
use rayon_types::{
    BrowserTab, CommandExecutionRequest, CommandExecutionResult, CommandInvocationResult,
    InteractiveSessionQueryRequest, InteractiveSessionState, InteractiveSessionSubmitRequest,
    InteractiveSessionSubmitResult, SearchResult, SearchResultKind, SearchableItemDocument,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tauri::AppHandle;

const BROWSER_TAB_SEARCH_LIMIT: usize = 20;

pub struct AppState {
    launcher: RwLock<LauncherService>,
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
    theme_settings: Arc<ThemeSettingsStore>,
    browser_tab_search_cache: Mutex<BrowserTabSearchCache>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app_index = Arc::new(
            TantivySearchIndex::open_or_create(paths::app_search_index_path(app)?)
                .map_err(|error| error.to_string())?,
        );
        let platform = Arc::new(MacOsAppManager);
        let theme_settings = Arc::new(ThemeSettingsStore::new(paths::app_theme_settings_path(
            app,
        )?));
        let launcher =
            launcher::build_launcher(platform.clone(), app_index.clone(), theme_settings.clone())?;

        Ok(Self {
            launcher: RwLock::new(launcher),
            platform,
            search_index: app_index,
            theme_settings,
            browser_tab_search_cache: Mutex::new(BrowserTabSearchCache::new()?),
        })
    }

    pub fn reload(&self) -> Result<CommandExecutionResult, String> {
        launcher::reload_launcher(
            &self.launcher,
            self.platform.clone(),
            self.search_index.clone(),
            self.theme_settings.clone(),
        )
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        self.read_launcher().search(query)
    }

    pub fn search_browser_tabs(&self, query: &str, refresh: bool) -> Vec<SearchResult> {
        match self.browser_tab_search_cache.lock() {
            Ok(mut cache) => cache.search(self.platform.as_ref(), query, refresh),
            Err(poisoned) => {
                eprintln!("browser tab search cache lock poisoned");
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

struct BrowserTabSearchCache {
    tabs_by_id: HashMap<String, BrowserTab>,
    ordered_tab_ids: Vec<String>,
    search_index: TantivySearchIndex,
    initialized: bool,
}

impl BrowserTabSearchCache {
    fn new() -> Result<Self, String> {
        Ok(Self {
            tabs_by_id: HashMap::new(),
            ordered_tab_ids: Vec::new(),
            search_index: TantivySearchIndex::create_in_memory()
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
                eprintln!("browser tab refresh failed: {error}");
                return Vec::new();
            }
        }

        let normalized_query = query.trim();
        if normalized_query.is_empty() {
            return self
                .ordered_tab_ids
                .iter()
                .take(BROWSER_TAB_SEARCH_LIMIT)
                .filter_map(|tab_id| self.tabs_by_id.get(tab_id))
                .cloned()
                .map(browser_tab_search_result)
                .collect();
        }

        let item_ids = match self
            .search_index
            .search_item_ids(normalized_query, BROWSER_TAB_SEARCH_LIMIT)
        {
            Ok(item_ids) => item_ids,
            Err(error) => {
                eprintln!("browser tab index search failed: {error}");
                return Vec::new();
            }
        };

        item_ids
            .into_iter()
            .filter_map(|item_id| self.tabs_by_id.get(&item_id))
            .cloned()
            .map(browser_tab_search_result)
            .collect()
    }

    fn refresh(&mut self, platform: &dyn AppPlatform) -> Result<(), String> {
        let tabs = platform.search_browser_tabs("")?;
        let documents = tabs
            .iter()
            .map(|tab| SearchableItemDocument {
                id: tab.command_id(),
                kind: SearchResultKind::BrowserTab,
                title: tab.title.clone(),
                subtitle: Some(tab.subtitle()),
                owner_plugin_id: None,
                search_text: tab.search_text(),
            })
            .collect::<Vec<_>>();

        self.search_index
            .replace_items(&documents)
            .map_err(|error| error.to_string())?;

        self.tabs_by_id = tabs
            .iter()
            .cloned()
            .map(|tab| (tab.command_id().to_string(), tab))
            .collect();
        self.ordered_tab_ids = tabs
            .into_iter()
            .map(|tab| tab.command_id().to_string())
            .collect();
        self.initialized = true;
        Ok(())
    }
}

fn browser_tab_search_result(tab: BrowserTab) -> SearchResult {
    let subtitle = tab.subtitle();
    SearchResult {
        id: tab.command_id(),
        title: tab.title,
        subtitle: Some(subtitle),
        icon_path: None,
        kind: SearchResultKind::BrowserTab,
        owner_plugin_id: None,
        keywords: Vec::new(),
        starts_interactive_session: false,
        close_launcher_on_success: true,
        input_mode: rayon_types::CommandInputMode::Structured,
        arguments: Vec::new(),
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
    use rayon_types::{BrowserTabTarget, CommandId, InstalledApp, ProcessMatch};
    use std::sync::Mutex;

    struct StubPlatform {
        browser_tabs: Mutex<Vec<BrowserTab>>,
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

        fn search_browser_tabs(&self, _query: &str) -> Result<Vec<BrowserTab>, String> {
            *self.search_calls.lock().unwrap() += 1;
            Ok(self.browser_tabs.lock().unwrap().clone())
        }

        fn focus_browser_tab(&self, _target: &BrowserTabTarget) -> Result<(), String> {
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

    #[test]
    fn browser_tab_search_cache_refreshes_only_on_request() {
        let platform = StubPlatform {
            browser_tabs: Mutex::new(vec![sample_tab(
                "Issue 15",
                "https://github.com/alexandrelam/rayon/issues/15",
                1,
                1,
            )]),
            search_calls: Mutex::new(0),
        };
        let mut cache = BrowserTabSearchCache::new().unwrap();

        let first_results = cache.search(&platform, "issue", true);
        let second_results = cache.search(&platform, "rayon", false);

        assert_eq!(first_results.len(), 1);
        assert_eq!(second_results.len(), 1);
        assert_eq!(*platform.search_calls.lock().unwrap(), 1);
    }

    #[test]
    fn browser_tab_search_cache_replaces_stale_tabs_on_refresh() {
        let platform = StubPlatform {
            browser_tabs: Mutex::new(vec![sample_tab("Old Tab", "https://old.example", 1, 1)]),
            search_calls: Mutex::new(0),
        };
        let mut cache = BrowserTabSearchCache::new().unwrap();

        let old_results = cache.search(&platform, "old", true);
        assert_eq!(old_results[0].title, "Old Tab");

        *platform.browser_tabs.lock().unwrap() =
            vec![sample_tab("Fresh Tab", "https://fresh.example/path", 1, 1)];

        let stale_results = cache.search(&platform, "fresh", false);
        assert!(stale_results.is_empty());

        let fresh_results = cache.search(&platform, "fresh", true);
        assert_eq!(fresh_results[0].title, "Fresh Tab");
        assert_eq!(*platform.search_calls.lock().unwrap(), 2);
    }

    #[test]
    fn browser_tab_search_cache_returns_all_tabs_for_blank_query() {
        let platform = StubPlatform {
            browser_tabs: Mutex::new(vec![
                sample_tab("Current", "https://current.example", 1, 1),
                sample_tab("Other", "https://other.example", 2, 1),
            ]),
            search_calls: Mutex::new(0),
        };
        let mut cache = BrowserTabSearchCache::new().unwrap();

        let results = cache.search(&platform, "", true);

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].id,
            CommandId::from("browser-tab:chrome:window-1:1")
        );
        assert_eq!(
            results[1].id,
            CommandId::from("browser-tab:chrome:window-2:1")
        );
    }
}
