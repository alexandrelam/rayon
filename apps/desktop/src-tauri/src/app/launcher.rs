use super::state::write_launcher;
use rayon_core::{
    load_config, AppPlatform, CommandRegistry, LauncherService, SearchIndex, APP_REINDEX_COMMAND_ID,
};
use rayon_features::{
    built_in_providers, BuiltInDependencies, ClipboardHistoryService, ThemeSettingsStore,
};
use rayon_types::{
    BookmarkDefinition, CommandExecutionRequest, CommandExecutionResult, CommandInvocationResult,
    ImageAssetDefinition,
};
use std::sync::{Arc, RwLock};

pub fn build_launcher(
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
    clipboard: Arc<ClipboardHistoryService>,
    theme_settings: Arc<ThemeSettingsStore>,
) -> Result<LauncherService, String> {
    let (registry, bookmarks, images) =
        load_registry_bookmarks_and_images(platform.clone(), clipboard, theme_settings)?;
    Ok(LauncherService::new(
        registry,
        bookmarks,
        images,
        platform,
        search_index,
    ))
}

pub fn reload_launcher(
    launcher_slot: &RwLock<LauncherService>,
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
    clipboard: Arc<ClipboardHistoryService>,
    theme_settings: Arc<ThemeSettingsStore>,
) -> Result<CommandExecutionResult, String> {
    let launcher = build_launcher(platform, search_index, clipboard, theme_settings)?;
    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: APP_REINDEX_COMMAND_ID.into(),
            argv: Default::default(),
            arguments: Default::default(),
        })
        .map_err(|error| error.to_string())?;
    let CommandInvocationResult::Completed { output } = result else {
        return Err("reindex unexpectedly started an interactive session".into());
    };
    *write_launcher(launcher_slot) = launcher;
    Ok(CommandExecutionResult { output })
}

fn load_registry_bookmarks_and_images(
    platform: Arc<dyn AppPlatform>,
    clipboard: Arc<ClipboardHistoryService>,
    theme_settings: Arc<ThemeSettingsStore>,
) -> Result<
    (
        CommandRegistry,
        Vec<BookmarkDefinition>,
        Vec<ImageAssetDefinition>,
    ),
    String,
> {
    let mut registry = CommandRegistry::new();
    let loaded_config = load_config().map_err(|error| error.to_string())?;

    for provider in built_in_providers(BuiltInDependencies {
        clipboard,
        platform,
        theme_settings,
    }) {
        registry
            .register_provider(provider)
            .map_err(|error| format!("failed to register built-in provider: {error}"))?;
    }

    for provider in loaded_config.command_providers {
        registry
            .register_provider(provider)
            .map_err(|error| error.to_string())?;
    }

    validate_bookmark_ids(&registry, &loaded_config.bookmarks)?;
    Ok((
        registry,
        loaded_config.bookmarks,
        loaded_config.image_assets,
    ))
}

fn validate_bookmark_ids(
    registry: &CommandRegistry,
    bookmarks: &[BookmarkDefinition],
) -> Result<(), String> {
    let command_ids = registry.search_results_by_id();
    for bookmark in bookmarks {
        if command_ids.contains_key(bookmark.id.as_str()) {
            return Err(format!(
                "bookmark id conflicts with an existing command id: {}",
                bookmark.id
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_core::SearchIndexStats;
    use rayon_features::ClipboardAccess;
    use rayon_types::{
        BrowserTab, BrowserTabTarget, CommandId, InstalledApp, ProcessMatch,
        SearchableItemDocument, ThemePreference,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct StubPlatform;

    struct StubClipboardAccess;

    impl ClipboardAccess for StubClipboardAccess {
        fn read_text(&self) -> Result<Option<String>, String> {
            Ok(None)
        }

        fn write_text(&self, _text: &str) -> Result<(), String> {
            Ok(())
        }
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
            Ok(Vec::new())
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

    #[derive(Default)]
    struct StubSearchIndex {
        documents: Mutex<Vec<SearchableItemDocument>>,
    }

    impl SearchIndex for StubSearchIndex {
        fn is_configured(&self) -> bool {
            true
        }

        fn search_item_ids(&self, query: &str, limit: usize) -> Result<Vec<String>, String> {
            let query = query.to_lowercase();
            Ok(self
                .documents
                .lock()
                .unwrap()
                .iter()
                .filter(|document| document.search_text.to_lowercase().contains(&query))
                .take(limit)
                .map(|document| document.id.to_string())
                .collect())
        }

        fn replace_items(
            &self,
            items: &[SearchableItemDocument],
        ) -> Result<SearchIndexStats, String> {
            *self.documents.lock().unwrap() = items.to_vec();
            Ok(SearchIndexStats {
                discovered_count: items.len(),
                indexed_count: items.len(),
                skipped_count: 0,
            })
        }
    }

    fn temp_config_home(test_name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rayon-{test_name}-{unique}"))
    }

    fn write_config(path: &Path, manifests: &[(&str, &str)]) {
        fs::create_dir_all(path.join("rayon")).unwrap();
        for (name, contents) in manifests {
            fs::write(path.join("rayon").join(name), contents).unwrap();
        }
    }

    struct LauncherTestContext {
        _env_guard: MutexGuard<'static, ()>,
        config_home: PathBuf,
        search_index: Arc<StubSearchIndex>,
        platform: Arc<dyn AppPlatform>,
        clipboard: Arc<ClipboardHistoryService>,
        theme_settings: Arc<ThemeSettingsStore>,
    }

    impl LauncherTestContext {
        fn new(test_name: &str, manifests: &[(&str, &str)]) -> Self {
            let env_guard = env_lock().lock().unwrap();
            let config_home = temp_config_home(test_name);
            write_config(&config_home, manifests);
            std::env::set_var("XDG_CONFIG_HOME", &config_home);

            let search_index = Arc::new(StubSearchIndex::default());
            let platform: Arc<dyn AppPlatform> = Arc::new(StubPlatform);
            let clipboard = Arc::new(
                ClipboardHistoryService::new(
                    Arc::new(StubClipboardAccess),
                    config_home.join("clipboard-history.json"),
                )
                .unwrap(),
            );
            let theme_settings = Arc::new(ThemeSettingsStore::new(config_home.join("theme.json")));

            Self {
                _env_guard: env_guard,
                config_home,
                search_index,
                platform,
                clipboard,
                theme_settings,
            }
        }

        fn build_launcher(&self) -> LauncherService {
            build_launcher(
                self.platform.clone(),
                self.search_index.clone(),
                self.clipboard.clone(),
                self.theme_settings.clone(),
            )
            .unwrap()
        }
    }

    impl Drop for LauncherTestContext {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.config_home);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn reload_launcher_picks_up_new_commands_and_bookmarks() {
        let context = LauncherTestContext::new(
            "reload-success",
            &[(
                "commands.toml",
                r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"
"#,
            )],
        );
        let launcher = context.build_launcher();
        let launcher_slot = RwLock::new(launcher);

        write_config(
            &context.config_home,
            &[
                (
                    "commands.toml",
                    r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"

[[commands]]
id = "user.ping"
title = "Ping"
program = "/bin/echo"
base_args = ["pong"]
"#,
                ),
                (
                    "bookmarks.toml",
                    r#"
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.jira"
title = "Jira Board"
url = "https://example.com/jira"
keywords = ["jira", "board"]
"#,
                ),
            ],
        );

        let result = reload_launcher(
            &launcher_slot,
            context.platform.clone(),
            context.search_index.clone(),
            context.clipboard.clone(),
            context.theme_settings.clone(),
        )
        .unwrap();
        assert!(result.output.starts_with("reindexed "));

        let launcher = launcher_slot.read().unwrap();
        let ping_results = launcher.search("ping");
        assert!(ping_results
            .iter()
            .any(|result| result.id == CommandId::from("user.ping")));

        let jira_results = launcher.search("jira");
        assert!(jira_results
            .iter()
            .any(|result| result.id == CommandId::from("user.jira")));
    }

    #[test]
    fn reload_launcher_keeps_previous_state_when_config_is_invalid() {
        let context = LauncherTestContext::new(
            "reload-failure",
            &[(
                "bookmarks.toml",
                r#"
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.docs"
title = "Docs"
url = "https://example.com/docs"
keywords = ["docs"]
"#,
            )],
        );
        let launcher = context.build_launcher();
        let launcher_slot = RwLock::new(launcher);

        write_config(
            &context.config_home,
            &[(
                "bookmarks.toml",
                r#"
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.docs"
title = "Broken"
url = "not-a-url"
"#,
            )],
        );

        let error = reload_launcher(
            &launcher_slot,
            context.platform.clone(),
            context.search_index.clone(),
            context.clipboard.clone(),
            context.theme_settings.clone(),
        )
        .unwrap_err();
        assert!(error.contains("invalid bookmark url"));

        let launcher = launcher_slot.read().unwrap();
        let results = launcher.search("docs");
        assert!(results
            .iter()
            .any(|result| result.id == CommandId::from("user.docs")));
    }

    #[test]
    fn build_launcher_rejects_bookmarks_that_conflict_with_command_ids() {
        let context = LauncherTestContext::new(
            "bookmark-command-conflict",
            &[
                (
                    "commands.toml",
                    r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"
"#,
                ),
                (
                    "bookmarks.toml",
                    r#"
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.echo"
title = "Echo Docs"
url = "https://example.com/echo"
"#,
                ),
            ],
        );

        let error = build_launcher(
            context.platform.clone(),
            context.search_index.clone(),
            context.clipboard.clone(),
            context.theme_settings.clone(),
        )
        .err()
        .unwrap();

        assert!(error.contains("bookmark id conflicts with an existing command id: user.echo"));
    }

    #[test]
    fn build_launcher_registers_theme_command() {
        let _env_guard = env_lock().lock().unwrap();
        let previous = std::env::var_os("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_HOME");

        let search_index = Arc::new(StubSearchIndex::default());
        let platform: Arc<dyn AppPlatform> = Arc::new(StubPlatform);
        let clipboard = Arc::new(
            ClipboardHistoryService::new(
                Arc::new(StubClipboardAccess),
                std::env::temp_dir().join("rayon-theme-command-clipboard.json"),
            )
            .unwrap(),
        );
        let theme_settings = Arc::new(ThemeSettingsStore::new(
            std::env::temp_dir().join("rayon-theme-command.json"),
        ));

        let launcher = build_launcher(platform, search_index, clipboard, theme_settings).unwrap();
        let results = launcher.search("theme");

        if let Some(previous) = previous {
            std::env::set_var("XDG_CONFIG_HOME", previous);
        }

        assert!(results
            .iter()
            .any(|result| result.id == CommandId::from("theme.set")));
    }

    #[test]
    fn theme_store_defaults_to_system_from_launcher_dependencies() {
        let path = std::env::temp_dir().join(format!(
            "rayon-theme-default-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = ThemeSettingsStore::new(path.clone());

        assert_eq!(store.load().unwrap(), ThemePreference::System);

        let _ = fs::remove_file(path);
    }
}
