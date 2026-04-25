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
    use rayon_types::CommandId;

    #[test]
    fn hello_provider_registers_and_executes() {
        let mut registry = CommandRegistry::new();
        for provider in built_in_providers() {
            registry.register_provider(provider).unwrap();
        }

        let results = registry.search("hello");
        assert_eq!(results.len(), 1);

        let execution = registry.execute(&CommandId::from("hello"), None).unwrap();
        assert_eq!(execution.output, "hello");
    }

    #[test]
    fn maintenance_provider_registers_reindex_command() {
        let mut registry = CommandRegistry::new();
        for provider in built_in_providers() {
            registry.register_provider(provider).unwrap();
        }

        let results = registry.search("reindex");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, CommandId::from("apps.reindex"));
    }
}
