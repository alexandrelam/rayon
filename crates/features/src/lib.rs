mod github;
mod kill;
mod maintenance;
mod theme;

use github::GitHubMyPrsProvider;
use rayon_core::{AppPlatform, CommandProvider};
use std::sync::Arc;
pub use theme::{ThemeCommandProvider, ThemeSettingsStore};

pub struct BuiltInDependencies {
    pub platform: Arc<dyn AppPlatform>,
    pub theme_settings: Arc<ThemeSettingsStore>,
}

pub fn built_in_providers(deps: BuiltInDependencies) -> Vec<Arc<dyn CommandProvider>> {
    vec![
        Arc::new(GitHubMyPrsProvider::new(deps.platform.clone())),
        Arc::new(kill::KillProvider::new(deps.platform)),
        Arc::new(maintenance::MaintenanceProvider),
        Arc::new(ThemeCommandProvider::new(deps.theme_settings)),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_core::CommandRegistry;
    use rayon_types::{BrowserTab, BrowserTabTarget, CommandId, ProcessMatch};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct StubPlatform;

    impl AppPlatform for StubPlatform {
        fn discover_apps(&self) -> Result<Vec<rayon_types::InstalledApp>, String> {
            Ok(Vec::new())
        }

        fn launch_app(&self, _app: &rayon_types::InstalledApp) -> Result<(), String> {
            Ok(())
        }

        fn open_url(&self, _url: &str) -> Result<(), String> {
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

    fn temp_theme_path(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rayon-features-theme-{test_name}-{unique}.json"))
    }

    fn built_ins() -> Vec<Arc<dyn CommandProvider>> {
        built_in_providers(BuiltInDependencies {
            platform: Arc::new(StubPlatform),
            theme_settings: Arc::new(ThemeSettingsStore::new(temp_theme_path("catalog"))),
        })
    }

    #[test]
    fn maintenance_provider_registers_reindex_command() {
        let mut registry = CommandRegistry::new();
        for provider in built_ins() {
            registry.register_provider(provider).unwrap();
        }

        let results = registry.search_results_by_id();

        assert_eq!(results["apps.reindex"].id, CommandId::from("apps.reindex"));
    }

    #[test]
    fn built_in_catalog_registers_theme_command() {
        let mut registry = CommandRegistry::new();
        for provider in built_ins() {
            registry.register_provider(provider).unwrap();
        }

        let results = registry.search_results_by_id();

        assert_eq!(results["theme.set"].id, CommandId::from("theme.set"));
    }

    #[test]
    fn built_in_catalog_registers_github_command() {
        let mut registry = CommandRegistry::new();
        for provider in built_ins() {
            registry.register_provider(provider).unwrap();
        }

        let results = registry.search_results_by_id();

        assert_eq!(
            results["github.my-prs"].id,
            CommandId::from("github.my-prs")
        );
    }
}
