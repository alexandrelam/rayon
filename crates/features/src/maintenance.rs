use rayon_core::{CommandError, CommandProvider, APP_REINDEX_COMMAND_ID};
use rayon_types::{
    CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId, CommandInputMode,
};

pub struct MaintenanceProvider;

impl CommandProvider for MaintenanceProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from(APP_REINDEX_COMMAND_ID),
            title: "Reindex Search".into(),
            subtitle: Some("Refresh apps, commands, bookmarks, and search".into()),
            owner_plugin_id: "builtin.maintenance".into(),
            keywords: vec!["refresh".into(), "index".into()],
            input_mode: CommandInputMode::Structured,
            arguments: Vec::new(),
        }]
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        if request.command_id.as_str() != APP_REINDEX_COMMAND_ID {
            return Err(CommandError::UnknownCommand(request.command_id.clone()));
        }

        Ok(CommandExecutionResult {
            output: "reindex is handled by the launcher service".into(),
        })
    }
}
