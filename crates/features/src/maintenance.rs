use rayon_core::{CommandError, CommandProvider, APP_REINDEX_COMMAND_ID};
use rayon_types::{CommandDefinition, CommandExecutionResult, CommandId};

pub struct MaintenanceProvider;

impl CommandProvider for MaintenanceProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from(APP_REINDEX_COMMAND_ID),
            title: "Reindex Applications".into(),
        }]
    }

    fn execute(
        &self,
        command_id: &CommandId,
        _payload: Option<String>,
    ) -> Result<CommandExecutionResult, CommandError> {
        if command_id.as_str() != APP_REINDEX_COMMAND_ID {
            return Err(CommandError::UnknownCommand(command_id.clone()));
        }

        Ok(CommandExecutionResult {
            output: "reindex is handled by the launcher service".into(),
        })
    }
}
