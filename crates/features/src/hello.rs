use rayon_core::{CommandError, CommandProvider};
use rayon_types::{CommandDefinition, CommandExecutionResult, CommandId};

pub struct HelloProvider;

impl CommandProvider for HelloProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from("hello"),
            title: "Hello".into(),
        }]
    }

    fn execute(
        &self,
        command_id: &CommandId,
        _payload: Option<String>,
    ) -> Result<CommandExecutionResult, CommandError> {
        if command_id.as_str() != "hello" {
            return Err(CommandError::UnknownCommand(command_id.clone()));
        }

        Ok(CommandExecutionResult {
            output: "hello".into(),
        })
    }
}
