mod hello;
mod maintenance;

use rayon_core::CommandProvider;
use std::sync::Arc;

pub fn built_in_providers() -> Vec<Arc<dyn CommandProvider>> {
    vec![
        Arc::new(hello::HelloProvider),
        Arc::new(maintenance::MaintenanceProvider),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon_core::CommandRegistry;
    use rayon_types::{CommandExecutionRequest, CommandId};
    use std::collections::HashMap;

    #[test]
    fn hello_provider_registers_and_executes() {
        let mut registry = CommandRegistry::new();
        for provider in built_in_providers() {
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
        for provider in built_in_providers() {
            registry.register_provider(provider).unwrap();
        }

        let results = registry.search_results_by_id();

        assert_eq!(results["apps.reindex"].id, CommandId::from("apps.reindex"));
    }
}
