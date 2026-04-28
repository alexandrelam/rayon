use super::error::LauncherError;
use super::routing::{resolve_execution_target, ExecutionTarget};
use super::service::LauncherService;
use super::state::{next_session_id, write_app_catalog, write_interactive_sessions};
use crate::catalog::AppCatalog;
use rayon_types::{
    CommandExecutionRequest, CommandExecutionResult, CommandId, CommandInvocationResult,
    InteractiveSessionMetadata, InteractiveSessionState,
};

impl LauncherService {
    pub fn execute_command(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandInvocationResult, LauncherError> {
        match resolve_execution_target(
            &request.command_id,
            self.bookmark_catalog.get(&request.command_id).is_some(),
            self.image_catalog.get(&request.command_id).is_some(),
        ) {
            ExecutionTarget::Reindex => {
                let result = self.refresh_and_reindex()?;
                Ok(CommandInvocationResult::Completed {
                    output: result.output,
                })
            }
            ExecutionTarget::App(command_id) => {
                let result = self.launch_app(&command_id)?;
                Ok(CommandInvocationResult::Completed {
                    output: result.output,
                })
            }
            ExecutionTarget::BrowserTab(target) => {
                let result = self.focus_browser_tab(&target)?;
                Ok(CommandInvocationResult::Completed {
                    output: result.output,
                })
            }
            ExecutionTarget::OpenWindow(target) => {
                let result = self.focus_open_window(&target)?;
                Ok(CommandInvocationResult::Completed {
                    output: result.output,
                })
            }
            ExecutionTarget::Bookmark(bookmark_id) => {
                let result = self.open_bookmark(&bookmark_id)?;
                Ok(CommandInvocationResult::Completed {
                    output: result.output,
                })
            }
            ExecutionTarget::Image(image_id) => {
                let result = self.copy_image(&image_id)?;
                Ok(CommandInvocationResult::Completed {
                    output: result.output,
                })
            }
            ExecutionTarget::Provider(command_id) => {
                if let Some(session_owner) = self.registry.start_interactive_session(&command_id)? {
                    return Ok(self.start_interactive_session(session_owner));
                }

                let result = self
                    .registry
                    .execute(request)
                    .map_err(LauncherError::from)?;
                Ok(CommandInvocationResult::Completed {
                    output: result.output,
                })
            }
        }
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

    fn focus_open_window(
        &self,
        target: &rayon_types::OpenWindowTarget,
    ) -> Result<CommandExecutionResult, LauncherError> {
        self.platform
            .focus_open_window(target)
            .map_err(LauncherError::Platform)?;

        Ok(CommandExecutionResult {
            output: "focused window".into(),
        })
    }

    fn copy_image(&self, image_id: &CommandId) -> Result<CommandExecutionResult, LauncherError> {
        let image = self
            .image_catalog
            .get(image_id)
            .cloned()
            .ok_or_else(|| LauncherError::AppNotFound(image_id.clone()))?;

        self.platform
            .copy_image_to_clipboard(std::path::Path::new(&image.path))
            .map_err(LauncherError::Platform)?;

        Ok(CommandExecutionResult {
            output: format!("copied {}", image.title),
        })
    }

    fn start_interactive_session(
        &self,
        session_owner: crate::commands::StartedInteractiveSession,
    ) -> CommandInvocationResult {
        let session_id = next_session_id(&self.next_session_id);
        let metadata = InteractiveSessionMetadata {
            session_id: session_id.clone(),
            command_id: session_owner.metadata.command_id.clone(),
            title: session_owner.metadata.title,
            subtitle: session_owner.metadata.subtitle,
            input_placeholder: session_owner.metadata.input_placeholder,
            completion_behavior: session_owner.metadata.completion_behavior,
        };

        write_interactive_sessions(&self.interactive_sessions).insert(
            session_id.clone(),
            super::state::ActiveInteractiveSession {
                provider_index: session_owner.provider_index,
                metadata: metadata.clone(),
            },
        );

        CommandInvocationResult::StartedSession {
            session: InteractiveSessionState {
                session_id,
                command_id: metadata.command_id,
                title: metadata.title,
                subtitle: metadata.subtitle,
                input_placeholder: metadata.input_placeholder,
                completion_behavior: metadata.completion_behavior,
                query: String::new(),
                is_loading: true,
                results: Vec::new(),
                message: None,
            },
        }
    }
}
