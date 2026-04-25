mod hello;
mod kill;
mod maintenance;

use rayon_core::{AppPlatform, CommandProvider};
use std::sync::Arc;

pub fn built_in_providers(platform: Arc<dyn AppPlatform>) -> Vec<Arc<dyn CommandProvider>> {
    vec![
        Arc::new(hello::HelloProvider),
        Arc::new(kill::KillProvider::new(platform)),
        Arc::new(maintenance::MaintenanceProvider),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_core::CommandRegistry;
    use rayon_types::ProcessMatch;
    use rayon_types::{CommandExecutionRequest, CommandId};
    use std::collections::HashMap;

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

        fn search_processes(&self, _query: &str) -> Result<Vec<ProcessMatch>, String> {
            Ok(Vec::new())
        }

        fn terminate_process(&self, _pid: u32) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn hello_provider_registers_and_executes() {
        let mut registry = CommandRegistry::new();
        for provider in built_in_providers(Arc::new(StubPlatform)) {
            registry.register_provider(provider).unwrap();
        }

        let results = registry.search_results_by_id();
        assert!(results.contains_key("hello"));

        let execution = registry
            .execute(&CommandExecutionRequest {
                command_id: CommandId::from("hello"),
                arguments: HashMap::new(),
            })
            .unwrap();
        assert_eq!(execution.output, "hello");
    }

    #[test]
    fn maintenance_provider_registers_reindex_command() {
        let mut registry = CommandRegistry::new();
        for provider in built_in_providers(Arc::new(StubPlatform)) {
            registry.register_provider(provider).unwrap();
        }

        let results = registry.search_results_by_id();

        assert_eq!(results["apps.reindex"].id, CommandId::from("apps.reindex"));
    }
}
