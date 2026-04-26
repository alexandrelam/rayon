use super::{launcher, paths};
use rayon_core::{AppPlatform, LauncherService, SearchIndex};
use rayon_db::TantivySearchIndex;
use rayon_features::ThemeSettingsStore;
use rayon_platform::MacOsAppManager;
use rayon_types::{
    CommandExecutionRequest, CommandExecutionResult, CommandInvocationResult,
    InteractiveSessionQueryRequest, InteractiveSessionState, InteractiveSessionSubmitRequest,
    InteractiveSessionSubmitResult, SearchResult,
};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tauri::AppHandle;

pub struct AppState {
    launcher: RwLock<LauncherService>,
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
    theme_settings: Arc<ThemeSettingsStore>,
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

pub fn write_launcher(launcher: &RwLock<LauncherService>) -> RwLockWriteGuard<'_, LauncherService> {
    match launcher.write() {
        Ok(launcher) => launcher,
        Err(poisoned) => {
            eprintln!("launcher lock poisoned while writing");
            poisoned.into_inner()
        }
    }
}
