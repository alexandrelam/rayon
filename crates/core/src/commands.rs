use rayon_types::{
    CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId,
    InteractiveSessionMetadata, InteractiveSessionResult, SearchResult, SearchResultKind,
    SearchableItemDocument,
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

pub const APP_REINDEX_COMMAND_ID: &str = "apps.reindex";

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

#[derive(Clone)]
pub(crate) struct StartedInteractiveSession {
    pub provider_index: usize,
    pub metadata: InteractiveSessionMetadata,
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

    pub(crate) fn start_interactive_session(
        &self,
        command_id: &CommandId,
    ) -> Result<Option<StartedInteractiveSession>, CommandError> {
        let provider_index = self
            .command_owners
            .get(command_id.as_str())
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(command_id.clone()))?;
        let provider = &self.providers[provider_index];
        let metadata = provider.start_interactive_session(command_id)?;
        Ok(metadata.map(|metadata| StartedInteractiveSession {
            provider_index,
            metadata,
        }))
    }

    pub(crate) fn search_interactive_session(
        &self,
        provider_index: usize,
        session: &InteractiveSessionMetadata,
        query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        self.providers[provider_index].search_interactive_session(session, query)
    }

    pub(crate) fn submit_interactive_session(
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
                        keywords: command.definition.keywords.clone(),
                        starts_interactive_session: command.starts_interactive_session,
                        input_mode: command.definition.input_mode,
                        arguments: command.definition.arguments.clone(),
                    },
                )
            })
            .collect()
    }

    pub(crate) fn searchable_documents(&self) -> Vec<SearchableItemDocument> {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::test_support::TestProvider;
    use std::collections::HashMap;
    use std::sync::Arc;

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
                command_id: CommandId::from("echo"),
                argv: Vec::new(),
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(result.output, "ran:echo");
    }

    #[test]
    fn returns_error_for_unknown_command() {
        let registry = CommandRegistry::new();

        let error = registry
            .execute(&CommandExecutionRequest {
                command_id: CommandId::from("missing"),
                argv: Vec::new(),
                arguments: HashMap::new(),
            })
            .unwrap_err();

        assert_eq!(
            error,
            CommandError::UnknownCommand(CommandId::from("missing"))
        );
    }
}
