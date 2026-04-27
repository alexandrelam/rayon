use rayon_core::{
    AppPlatform, CommandError, CommandProvider, InteractiveSessionSubmitOutcome,
    InteractiveSessionUpdate,
};
use rayon_types::{
    CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId,
    CommandInputMode, InteractiveSessionCompletionBehavior, InteractiveSessionMetadata,
    InteractiveSessionResult,
};
use std::sync::Arc;

const KILL_COMMAND_ID: &str = "kill";

pub struct KillProvider {
    platform: Arc<dyn AppPlatform>,
}

impl KillProvider {
    pub fn new(platform: Arc<dyn AppPlatform>) -> Self {
        Self { platform }
    }
}

impl CommandProvider for KillProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from(KILL_COMMAND_ID),
            title: "Kill Process".into(),
            subtitle: Some("Terminate a running process or port owner".into()),
            owner_plugin_id: "builtin.kill".into(),
            keywords: vec![
                "kill".into(),
                "process".into(),
                "port".into(),
                "terminate".into(),
            ],
            close_launcher_on_success: false,
            input_mode: CommandInputMode::Structured,
            arguments: Vec::new(),
        }]
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        Err(CommandError::UnknownCommand(request.command_id.clone()))
    }

    fn start_interactive_session(
        &self,
        command_id: &CommandId,
    ) -> Result<Option<InteractiveSessionMetadata>, CommandError> {
        if command_id.as_str() != KILL_COMMAND_ID {
            return Ok(None);
        }

        Ok(Some(InteractiveSessionMetadata {
            session_id: String::new(),
            command_id: command_id.clone(),
            title: "Kill Process".into(),
            subtitle: Some("Search by app, process, or port".into()),
            input_placeholder: "Search process name or port 8080".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        }))
    }

    fn search_interactive_session(
        &self,
        session: &InteractiveSessionMetadata,
        query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        if session.command_id.as_str() != KILL_COMMAND_ID {
            return Err(CommandError::UnknownCommand(session.command_id.clone()));
        }

        let matches = self
            .platform
            .search_processes(query)
            .map_err(CommandError::ExecutionFailed)?;

        Ok(matches.into_iter().map(to_session_result).collect())
    }

    fn submit_interactive_session(
        &self,
        session: &InteractiveSessionMetadata,
        query: &str,
        item_id: &str,
    ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
        if session.command_id.as_str() != KILL_COMMAND_ID {
            return Err(CommandError::UnknownCommand(session.command_id.clone()));
        }

        let pid = item_id.parse::<u32>().map_err(|_| {
            CommandError::InvalidArguments(format!("invalid process id: {item_id}"))
        })?;
        self.platform
            .terminate_process(pid)
            .map_err(CommandError::ExecutionFailed)?;

        let refreshed_results = self
            .platform
            .search_processes(query)
            .map_err(CommandError::ExecutionFailed)?;

        Ok(InteractiveSessionSubmitOutcome::Updated(
            InteractiveSessionUpdate {
                results: refreshed_results
                    .into_iter()
                    .filter(|process_match| process_match.pid != pid)
                    .map(to_session_result)
                    .collect(),
                message: Some(format!("terminated PID {pid}")),
            },
        ))
    }
}

fn to_session_result(process_match: rayon_types::ProcessMatch) -> InteractiveSessionResult {
    let mut subtitle = format!("PID {} · {}", process_match.pid, process_match.command);
    if !process_match.matched_ports.is_empty() {
        let ports = process_match
            .matched_ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        subtitle.push_str(&format!(" · ports {ports}"));
    }

    InteractiveSessionResult {
        id: process_match.pid.to_string(),
        title: process_match.display_name,
        subtitle: Some(subtitle),
    }
}
