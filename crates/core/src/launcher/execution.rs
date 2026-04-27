use super::error::LauncherError;
use super::service::LauncherService;
use super::state::{next_session_id, write_app_catalog, write_interactive_sessions};
use crate::catalog::AppCatalog;
use crate::commands::APP_REINDEX_COMMAND_ID;
use rayon_types::{
    parse_browser_tab_command_id, CommandExecutionRequest, CommandExecutionResult, CommandId,
    CommandInvocationResult, InteractiveSessionMetadata, InteractiveSessionState,
};

impl LauncherService {
    pub fn execute_command(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandInvocationResult, LauncherError> {
        if request.command_id.as_str() == APP_REINDEX_COMMAND_ID {
            let result = self.refresh_and_reindex()?;
            return Ok(CommandInvocationResult::Completed {
                output: result.output,
            });
        }

        if request.command_id.as_str().starts_with("app:macos:") {
            let result = self.launch_app(&request.command_id)?;
            return Ok(CommandInvocationResult::Completed {
                output: result.output,
            });
        }

        if let Some(target) = parse_browser_tab_command_id(&request.command_id) {
            let result = self.focus_browser_tab(&target)?;
            return Ok(CommandInvocationResult::Completed {
                output: result.output,
            });
        }

        if self.bookmark_catalog.get(&request.command_id).is_some() {
            let result = self.open_bookmark(&request.command_id)?;
            return Ok(CommandInvocationResult::Completed {
                output: result.output,
            });
        }

        if let Some(session_owner) = self
            .registry
            .start_interactive_session(&request.command_id)?
        {
            let session_id = next_session_id(&self.next_session_id);
            let metadata = InteractiveSessionMetadata {
                session_id: session_id.clone(),
                command_id: session_owner.metadata.command_id.clone(),
                title: session_owner.metadata.title,
                subtitle: session_owner.metadata.subtitle,
                input_placeholder: session_owner.metadata.input_placeholder,
            };

            write_interactive_sessions(&self.interactive_sessions).insert(
                session_id.clone(),
                super::state::ActiveInteractiveSession {
                    provider_index: session_owner.provider_index,
                    metadata: metadata.clone(),
                },
            );

            return Ok(CommandInvocationResult::StartedSession {
                session: InteractiveSessionState {
                    session_id,
                    command_id: metadata.command_id,
                    title: metadata.title,
                    subtitle: metadata.subtitle,
                    input_placeholder: metadata.input_placeholder,
                    query: String::new(),
                    is_loading: true,
                    results: Vec::new(),
                    message: None,
                },
            });
        }

        let result = self
            .registry
            .execute(request)
            .map_err(LauncherError::from)?;
        Ok(CommandInvocationResult::Completed {
            output: result.output,
        })
    }

    fn refresh_and_reindex(&self) -> Result<CommandExecutionResult, LauncherError> {
        let apps = self
            .platform
            .discover_apps()
            .map_err(LauncherError::Platform)?;
        {
            let mut app_catalog = write_app_catalog(&self.app_catalog);
            *app_catalog = AppCatalog::from_apps(apps);
        }

        let stats = self
            .reindex_search()
            .map_err(LauncherError::SearchBackend)?;
        Ok(CommandExecutionResult {
            output: format!(
                "reindexed {} searchable items ({} skipped)",
                stats.indexed_count, stats.skipped_count
            ),
        })
    }

    fn launch_app(&self, command_id: &CommandId) -> Result<CommandExecutionResult, LauncherError> {
        let app = {
            let app_catalog = super::state::read_app_catalog(&self.app_catalog);
            app_catalog
                .get(command_id)
                .cloned()
                .ok_or_else(|| LauncherError::AppNotFound(command_id.clone()))?
        };

        self.platform
            .launch_app(&app)
            .map_err(LauncherError::Platform)?;

        Ok(CommandExecutionResult {
            output: format!("opened {}", app.title),
        })
    }

    fn open_bookmark(
        &self,
        bookmark_id: &CommandId,
    ) -> Result<CommandExecutionResult, LauncherError> {
        let bookmark = self
            .bookmark_catalog
            .get(bookmark_id)
            .cloned()
            .ok_or_else(|| LauncherError::AppNotFound(bookmark_id.clone()))?;

        self.platform
            .open_url(&bookmark.url)
            .map_err(LauncherError::Platform)?;

        Ok(CommandExecutionResult {
            output: format!("opened {}", bookmark.title),
        })
    }

    fn focus_browser_tab(
        &self,
        target: &rayon_types::BrowserTabTarget,
    ) -> Result<CommandExecutionResult, LauncherError> {
        self.platform
            .focus_browser_tab(target)
            .map_err(LauncherError::Platform)?;

        Ok(CommandExecutionResult {
            output: "focused Chrome tab".into(),
        })
    }
}
