use rayon_core::{CommandError, CommandProvider};
use rayon_types::{CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId};

pub struct HelloProvider;

impl CommandProvider for HelloProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from("hello"),
            title: "Hello".into(),
            subtitle: Some("Built-in greeting".into()),
            owner_plugin_id: "builtin.hello".into(),
            keywords: vec!["greet".into(), "hello".into()],
            arguments: Vec::new(),
        }]
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        if request.command_id.as_str() != "hello" {
            return Err(CommandError::UnknownCommand(request.command_id.clone()));
        }

        Ok(CommandExecutionResult {
            output: "hello".into(),
        })
    }
}
