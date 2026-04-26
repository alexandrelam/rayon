use super::error::LauncherError;
use super::service::LauncherService;
use super::state::{
    read_interactive_sessions, write_interactive_sessions, ActiveInteractiveSession,
};
use crate::commands::InteractiveSessionSubmitOutcome;
use rayon_types::{
    InteractiveSessionQueryRequest, InteractiveSessionState, InteractiveSessionSubmitRequest,
    InteractiveSessionSubmitResult,
};

impl LauncherService {
    pub fn search_interactive_session(
        &self,
        request: &InteractiveSessionQueryRequest,
    ) -> Result<InteractiveSessionState, LauncherError> {
        let session = self.active_session(&request.session_id)?;
        let results = self.registry.search_interactive_session(
            session.provider_index,
            &session.metadata,
            &request.query,
        )?;

        Ok(InteractiveSessionState {
            session_id: session.metadata.session_id,
            command_id: session.metadata.command_id,
            title: session.metadata.title,
            subtitle: session.metadata.subtitle,
            input_placeholder: session.metadata.input_placeholder,
            query: request.query.clone(),
            is_loading: false,
            results,
            message: None,
        })
    }

    pub fn submit_interactive_session(
        &self,
        request: &InteractiveSessionSubmitRequest,
    ) -> Result<InteractiveSessionSubmitResult, LauncherError> {
        let session = self.active_session(&request.session_id)?;
        let outcome = self.registry.submit_interactive_session(
            session.provider_index,
            &session.metadata,
            &request.query,
            &request.item_id,
        )?;

        match outcome {
            InteractiveSessionSubmitOutcome::Updated(update) => {
                Ok(InteractiveSessionSubmitResult::UpdatedSession {
                    session: InteractiveSessionState {
                        session_id: session.metadata.session_id,
                        command_id: session.metadata.command_id,
                        title: session.metadata.title,
                        subtitle: session.metadata.subtitle,
                        input_placeholder: session.metadata.input_placeholder,
                        query: request.query.clone(),
                        is_loading: false,
                        results: update.results,
                        message: update.message,
                    },
                })
            }
            InteractiveSessionSubmitOutcome::Completed(result) => {
                write_interactive_sessions(&self.interactive_sessions).remove(&request.session_id);
                Ok(InteractiveSessionSubmitResult::Completed {
                    output: result.output,
                })
            }
        }
    }

    fn active_session(&self, session_id: &str) -> Result<ActiveInteractiveSession, LauncherError> {
        read_interactive_sessions(&self.interactive_sessions)
            .get(session_id)
            .cloned()
            .ok_or_else(|| LauncherError::InteractiveSessionNotFound(session_id.to_string()))
    }
}
