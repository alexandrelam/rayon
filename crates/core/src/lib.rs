use rayon_types::{CommandDefinition, CommandExecutionResult, CommandId, SearchResult};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

pub trait CommandProvider: Send + Sync {
    fn commands(&self) -> Vec<CommandDefinition>;
    fn execute(
        &self,
        command_id: &CommandId,
        payload: Option<String>,
    ) -> Result<CommandExecutionResult, CommandError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    DuplicateCommandId(CommandId),
    UnknownCommand(CommandId),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCommandId(command_id) => {
                write!(f, "duplicate command id registered: {command_id}")
            }
            Self::UnknownCommand(command_id) => write!(f, "unknown command id: {command_id}"),
        }
    }
}

impl Error for CommandError {}

pub struct CommandRegistry {
    providers: Vec<Arc<dyn CommandProvider>>,
    commands: Vec<RegisteredCommand>,
    command_owners: HashMap<String, usize>,
}

struct RegisteredCommand {
    definition: CommandDefinition,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            commands: Vec::new(),
            command_owners: HashMap::new(),
        }
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

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query = query.trim().to_ascii_lowercase();

        self.commands
            .iter()
            .filter(|command| {
                if query.is_empty() {
                    return true;
                }

                let title = command.definition.title.to_ascii_lowercase();
                let id = command.definition.id.as_str().to_ascii_lowercase();

                title.contains(&query) || id.contains(&query)
            })
            .map(|command| SearchResult {
                id: command.definition.id.clone(),
                title: command.definition.title.clone(),
            })
            .collect()
    }

    pub fn execute(
        &self,
        command_id: &CommandId,
        payload: Option<String>,
    ) -> Result<CommandExecutionResult, CommandError> {
        let provider_index = self
            .command_owners
            .get(command_id.as_str())
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(command_id.clone()))?;

        self.providers[provider_index].execute(command_id, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider;

    impl CommandProvider for TestProvider {
        fn commands(&self) -> Vec<CommandDefinition> {
            vec![
                CommandDefinition {
                    id: CommandId::from("hello"),
                    title: "Hello".into(),
                },
                CommandDefinition {
                    id: CommandId::from("help"),
                    title: "Help".into(),
                },
            ]
        }

        fn execute(
            &self,
            command_id: &CommandId,
            _payload: Option<String>,
        ) -> Result<CommandExecutionResult, CommandError> {
            Ok(CommandExecutionResult {
                output: format!("ran:{command_id}"),
            })
        }
    }

    #[test]
    fn returns_all_commands_for_empty_query() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let results = registry.search("");

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn filters_commands_case_insensitively() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let results = registry.search("HEL");

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn rejects_duplicate_command_ids() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let error = registry
            .register_provider(Arc::new(TestProvider))
            .unwrap_err();

        assert_eq!(
            error,
            CommandError::DuplicateCommandId(CommandId::from("hello"))
        );
    }

    #[test]
    fn executes_command_through_provider() {
        let mut registry = CommandRegistry::new();
        registry.register_provider(Arc::new(TestProvider)).unwrap();

        let result = registry.execute(&CommandId::from("hello"), None).unwrap();

        assert_eq!(result.output, "ran:hello");
    }

    #[test]
    fn returns_error_for_unknown_command() {
        let registry = CommandRegistry::new();

        let error = registry
            .execute(&CommandId::from("missing"), None)
            .unwrap_err();

        assert_eq!(
            error,
            CommandError::UnknownCommand(CommandId::from("missing"))
        );
    }
}
