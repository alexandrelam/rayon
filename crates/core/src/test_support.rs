#![allow(clippy::unwrap_used)]

use crate::{
    AppPlatform, CommandError, CommandProvider, CommandRegistry, InteractiveSessionSubmitOutcome,
    InteractiveSessionUpdate, LauncherService, SearchIndex, SearchIndexStats,
};
use rayon_types::{
    BookmarkDefinition, BrowserTab, BrowserTabTarget, CommandArgumentDefinition,
    CommandArgumentType, CommandDefinition, CommandExecutionRequest, CommandExecutionResult,
    CommandId, CommandInputMode, InstalledApp, InteractiveSessionCompletionBehavior,
    InteractiveSessionMetadata, InteractiveSessionResult, ProcessMatch, SearchableItemDocument,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(crate) struct TestProvider;

impl CommandProvider for TestProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![
            CommandDefinition {
                id: CommandId::from("echo"),
                title: "Echo".into(),
                subtitle: None,
                owner_plugin_id: "builtin.echo".into(),
                keywords: vec![],
                close_launcher_on_success: false,
                input_mode: CommandInputMode::Structured,
                arguments: vec![CommandArgumentDefinition {
                    id: String::from("message"),
                    label: String::from("Message"),
                    argument_type: CommandArgumentType::String,
                    required: true,
                    flag: None,
                    positional: Some(0),
                    default_value: None,
                }],
            },
            CommandDefinition {
                id: CommandId::from("kill"),
                title: "Kill Process".into(),
                subtitle: Some("Terminate a running process".into()),
                owner_plugin_id: "builtin.kill".into(),
                keywords: vec!["terminate".into()],
                close_launcher_on_success: false,
                input_mode: CommandInputMode::Structured,
                arguments: vec![],
            },
        ]
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        Ok(CommandExecutionResult {
            output: format!("ran:{}", request.command_id),
        })
    }

    fn start_interactive_session(
        &self,
        command_id: &CommandId,
    ) -> Result<Option<InteractiveSessionMetadata>, CommandError> {
        if command_id.as_str() != "kill" {
            return Ok(None);
        }

        Ok(Some(InteractiveSessionMetadata {
            session_id: String::new(),
            command_id: command_id.clone(),
            title: "Kill Process".into(),
            subtitle: Some("Terminate a running process".into()),
            input_placeholder: "Search by process name or port".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        }))
    }

    fn search_interactive_session(
        &self,
        _session: &InteractiveSessionMetadata,
        query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        Ok(vec![InteractiveSessionResult {
            id: format!("result:{query}"),
            title: "Arc".into(),
            subtitle: Some("PID 1234".into()),
        }])
    }

    fn submit_interactive_session(
        &self,
        _session: &InteractiveSessionMetadata,
        query: &str,
        item_id: &str,
    ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
        Ok(InteractiveSessionSubmitOutcome::Updated(
            InteractiveSessionUpdate {
                results: vec![InteractiveSessionResult {
                    id: format!("refresh:{query}"),
                    title: "Preview".into(),
                    subtitle: Some("PID 99".into()),
                }],
                message: Some(format!("terminated {item_id}")),
            },
        ))
    }
}

pub(crate) struct CompletingProvider;

impl CommandProvider for CompletingProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from("github.my-prs"),
            title: "My Pull Requests".into(),
            subtitle: Some("Open a pull request".into()),
            owner_plugin_id: "builtin.github".into(),
            keywords: vec!["github".into()],
            close_launcher_on_success: false,
            input_mode: CommandInputMode::Structured,
            arguments: vec![],
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
        if command_id.as_str() != "github.my-prs" {
            return Ok(None);
        }

        Ok(Some(InteractiveSessionMetadata {
            session_id: String::new(),
            command_id: command_id.clone(),
            title: "My Pull Requests".into(),
            subtitle: Some("Open a pull request".into()),
            input_placeholder: "Filter".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        }))
    }

    fn search_interactive_session(
        &self,
        _session: &InteractiveSessionMetadata,
        _query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        Ok(vec![InteractiveSessionResult {
            id: "https://github.com/example/repo/pull/1".into(),
            title: "Fix bug".into(),
            subtitle: Some("example/repo #1".into()),
        }])
    }

    fn submit_interactive_session(
        &self,
        _session: &InteractiveSessionMetadata,
        _query: &str,
        _item_id: &str,
    ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
        Ok(InteractiveSessionSubmitOutcome::Completed(
            CommandExecutionResult {
                output: "opened example/repo#1".into(),
            },
        ))
    }
}

pub(crate) struct StubPlatform {
    pub apps: Vec<InstalledApp>,
    pub launched: Mutex<Vec<String>>,
    pub opened_urls: Mutex<Vec<String>>,
    pub copied_images: Mutex<Vec<String>>,
    pub browser_tabs: Mutex<Vec<BrowserTab>>,
    pub focused_browser_tabs: Mutex<Vec<BrowserTabTarget>>,
    pub process_search_results: Mutex<Vec<ProcessMatch>>,
    pub terminated_pids: Mutex<Vec<u32>>,
}

impl AppPlatform for StubPlatform {
    fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
        Ok(self.apps.clone())
    }

    fn launch_app(&self, app: &InstalledApp) -> Result<(), String> {
        self.launched.lock().unwrap().push(app.id.to_string());
        Ok(())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        self.opened_urls.lock().unwrap().push(url.to_string());
        Ok(())
    }

    fn copy_image_to_clipboard(&self, image_path: &Path) -> Result<(), String> {
        self.copied_images
            .lock()
            .unwrap()
            .push(image_path.display().to_string());
        Ok(())
    }

    fn search_browser_tabs(&self, query: &str) -> Result<Vec<BrowserTab>, String> {
        let normalized_query = query.trim().to_lowercase();
        Ok(self
            .browser_tabs
            .lock()
            .unwrap()
            .iter()
            .filter(|tab| tab.search_text().contains(&normalized_query))
            .cloned()
            .collect())
    }

    fn focus_browser_tab(&self, target: &BrowserTabTarget) -> Result<(), String> {
        self.focused_browser_tabs
            .lock()
            .unwrap()
            .push(target.clone());
        Ok(())
    }

    fn search_processes(&self, _query: &str) -> Result<Vec<ProcessMatch>, String> {
        Ok(self.process_search_results.lock().unwrap().clone())
    }

    fn terminate_process(&self, pid: u32) -> Result<(), String> {
        self.terminated_pids.lock().unwrap().push(pid);
        Ok(())
    }
}

pub(crate) struct StubSearchIndex {
    pub configured: bool,
    pub search_results: Vec<String>,
    pub stats: SearchIndexStats,
    pub last_documents: Mutex<Vec<SearchableItemDocument>>,
}

impl SearchIndex for StubSearchIndex {
    fn is_configured(&self) -> bool {
        self.configured
    }

    fn search_item_ids(&self, _query: &str, _limit: usize) -> Result<Vec<String>, String> {
        Ok(self.search_results.clone())
    }

    fn replace_items(&self, items: &[SearchableItemDocument]) -> Result<SearchIndexStats, String> {
        *self.last_documents.lock().unwrap() = items.to_vec();
        Ok(SearchIndexStats {
            discovered_count: items.len(),
            ..self.stats.clone()
        })
    }
}

pub(crate) fn build_launcher_service(search_results: Vec<String>) -> LauncherService {
    let mut registry = CommandRegistry::new();
    registry.register_provider(Arc::new(TestProvider)).unwrap();

    let platform = Arc::new(StubPlatform {
        apps: vec![InstalledApp {
            id: CommandId::from("app:macos:com.example.arc"),
            title: "Arc".into(),
            bundle_identifier: Some("com.example.arc".into()),
            path: "/Applications/Arc.app".into(),
        }],
        launched: Mutex::new(Vec::new()),
        opened_urls: Mutex::new(Vec::new()),
        copied_images: Mutex::new(Vec::new()),
        browser_tabs: Mutex::new(Vec::new()),
        focused_browser_tabs: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let index = Arc::new(StubSearchIndex {
        configured: true,
        search_results,
        stats: SearchIndexStats {
            discovered_count: 0,
            indexed_count: 5,
            skipped_count: 0,
        },
        last_documents: Mutex::new(Vec::new()),
    });

    LauncherService::new(
        registry,
        vec![BookmarkDefinition {
            id: CommandId::from("bookmark:github"),
            title: "GitHub".into(),
            subtitle: Some("Code hosting".into()),
            owner_plugin_id: "user.links".into(),
            url: "https://github.com".into(),
            keywords: vec!["git".into(), "repos".into()],
        }],
        Vec::new(),
        platform,
        index,
    )
}
