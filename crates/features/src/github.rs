use rayon_core::{AppPlatform, CommandError, CommandProvider, InteractiveSessionSubmitOutcome};
use rayon_types::{
    CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId,
    CommandInputMode, InteractiveSessionCompletionBehavior, InteractiveSessionMetadata,
    InteractiveSessionResult,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};

const GITHUB_MY_PRS_COMMAND_ID: &str = "github.my-prs";

pub struct GitHubMyPrsProvider {
    platform: Arc<dyn AppPlatform>,
    cli: Arc<dyn GitHubCli>,
    session_prs: Mutex<HashMap<String, Vec<GitHubPullRequest>>>,
}

impl GitHubMyPrsProvider {
    pub fn new(platform: Arc<dyn AppPlatform>) -> Self {
        Self::with_cli(platform, Arc::new(SystemGitHubCli))
    }

    fn with_cli(platform: Arc<dyn AppPlatform>, cli: Arc<dyn GitHubCli>) -> Self {
        Self {
            platform,
            cli,
            session_prs: Mutex::new(HashMap::new()),
        }
    }

    fn session_prs(
        &self,
        session: &InteractiveSessionMetadata,
    ) -> Result<Vec<GitHubPullRequest>, CommandError> {
        let session_prs = self
            .session_prs
            .lock()
            .map_err(|_| CommandError::ExecutionFailed("GitHub PR cache lock poisoned".into()))?;

        if let Some(prs) = session_prs.get(&session.session_id) {
            return Ok(prs.clone());
        }
        drop(session_prs);

        let prs = self
            .cli
            .search_my_open_prs()
            .map_err(CommandError::ExecutionFailed)?;

        let mut session_prs = self
            .session_prs
            .lock()
            .map_err(|_| CommandError::ExecutionFailed("GitHub PR cache lock poisoned".into()))?;
        if let Some(cached_prs) = session_prs.get(&session.session_id) {
            return Ok(cached_prs.clone());
        }
        session_prs.insert(session.session_id.clone(), prs.clone());
        Ok(prs)
    }

    fn clear_session(&self, session_id: &str) {
        if let Ok(mut session_prs) = self.session_prs.lock() {
            session_prs.remove(session_id);
        }
    }
}

impl CommandProvider for GitHubMyPrsProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from(GITHUB_MY_PRS_COMMAND_ID),
            title: "My Pull Requests".into(),
            subtitle: Some("Search your open GitHub pull requests".into()),
            owner_plugin_id: "builtin.github".into(),
            keywords: vec![
                "github".into(),
                "gh".into(),
                "pr".into(),
                "pull request".into(),
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
        if command_id.as_str() != GITHUB_MY_PRS_COMMAND_ID {
            return Ok(None);
        }

        Ok(Some(InteractiveSessionMetadata {
            session_id: String::new(),
            command_id: command_id.clone(),
            title: "My Pull Requests".into(),
            subtitle: Some("Open one of your authored pull requests".into()),
            input_placeholder: "Filter by title, repository, or number".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        }))
    }

    fn search_interactive_session(
        &self,
        session: &InteractiveSessionMetadata,
        query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        if session.command_id.as_str() != GITHUB_MY_PRS_COMMAND_ID {
            return Err(CommandError::UnknownCommand(session.command_id.clone()));
        }

        let prs = self.session_prs(session)?;
        Ok(filter_pull_requests(&prs, query)
            .into_iter()
            .map(to_session_result)
            .collect())
    }

    fn submit_interactive_session(
        &self,
        session: &InteractiveSessionMetadata,
        _query: &str,
        item_id: &str,
    ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
        if session.command_id.as_str() != GITHUB_MY_PRS_COMMAND_ID {
            return Err(CommandError::UnknownCommand(session.command_id.clone()));
        }

        let prs = self.session_prs(session)?;
        let selected = prs.iter().find(|pr| pr.url == item_id).ok_or_else(|| {
            CommandError::InvalidArguments(format!("unknown pull request: {item_id}"))
        })?;

        self.platform
            .open_url(&selected.url)
            .map_err(CommandError::ExecutionFailed)?;

        Ok(InteractiveSessionSubmitOutcome::Completed(
            CommandExecutionResult {
                output: format!("opened {}", selected.display_ref()),
            },
        ))
    }

    fn end_interactive_session(&self, session: &InteractiveSessionMetadata) {
        self.clear_session(&session.session_id);
    }
}

trait GitHubCli: Send + Sync {
    fn search_my_open_prs(&self) -> Result<Vec<GitHubPullRequest>, String>;
}

struct SystemGitHubCli;

impl GitHubCli for SystemGitHubCli {
    fn search_my_open_prs(&self) -> Result<Vec<GitHubPullRequest>, String> {
        ensure_gh_authenticated()?;
        let output = run_gh(&[
            "search",
            "prs",
            "--author",
            "@me",
            "--state",
            "open",
            "--sort",
            "updated",
            "--order",
            "desc",
            "--limit",
            "100",
            "--json",
            "title,url,repository,number,updatedAt,isDraft",
        ])?;

        if !output.status.success() {
            return Err(stderr_or_stdout(&output)
                .unwrap_or_else(|| "GitHub CLI failed to list pull requests".to_string()));
        }

        serde_json::from_slice::<Vec<GitHubPullRequest>>(&output.stdout)
            .map_err(|error| format!("failed to parse GitHub CLI output: {error}"))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct GitHubPullRequest {
    title: String,
    url: String,
    repository: GitHubRepository,
    number: u64,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
}

impl GitHubPullRequest {
    fn status_label(&self) -> &'static str {
        if self.is_draft {
            "draft"
        } else {
            "open"
        }
    }

    fn updated_date(&self) -> &str {
        self.updated_at
            .split('T')
            .next()
            .unwrap_or(&self.updated_at)
    }

    fn display_ref(&self) -> String {
        format!("{}#{}", self.repository.name_with_owner, self.number)
    }

    fn search_text(&self) -> String {
        format!(
            "{} {} #{} {}",
            self.title,
            self.repository.name_with_owner,
            self.number,
            self.status_label()
        )
        .to_lowercase()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct GitHubRepository {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

fn to_session_result(pr: GitHubPullRequest) -> InteractiveSessionResult {
    let subtitle = format!(
        "{} #{} · {} · updated {}",
        pr.repository.name_with_owner,
        pr.number,
        pr.status_label(),
        pr.updated_date()
    );

    InteractiveSessionResult {
        id: pr.url.clone(),
        title: pr.title,
        subtitle: Some(subtitle),
    }
}

fn filter_pull_requests(prs: &[GitHubPullRequest], query: &str) -> Vec<GitHubPullRequest> {
    let normalized_query = query.trim().to_lowercase();
    prs.iter()
        .filter(|pr| normalized_query.is_empty() || pr.search_text().contains(&normalized_query))
        .cloned()
        .collect()
}

fn ensure_gh_authenticated() -> Result<(), String> {
    let output = run_gh(&["auth", "status", "--active"])?;
    if output.status.success() {
        return Ok(());
    }

    let message = stderr_or_stdout(&output).unwrap_or_default();
    if message.contains("not logged into any GitHub hosts") || message.contains("gh auth login") {
        return Err("GitHub CLI is not authenticated. Run gh auth login.".into());
    }

    Err(if message.is_empty() {
        "GitHub CLI is not authenticated. Run gh auth login.".into()
    } else {
        message
    })
}

fn run_gh(args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("gh").args(args).output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "GitHub CLI (gh) is not installed or not on PATH.".into()
        } else {
            format!("failed to run gh: {error}")
        }
    })
}

fn stderr_or_stdout(output: &std::process::Output) -> Option<String> {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return Some(stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_core::InteractiveSessionSubmitOutcome;
    use rayon_types::{BrowserTab, BrowserTabTarget, CommandId, InstalledApp, ProcessMatch};
    use std::collections::VecDeque;

    #[derive(Default)]
    struct StubPlatform {
        opened_urls: Mutex<Vec<String>>,
    }

    impl AppPlatform for StubPlatform {
        fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
            Ok(Vec::new())
        }

        fn launch_app(&self, _app: &InstalledApp) -> Result<(), String> {
            Ok(())
        }

        fn open_url(&self, url: &str) -> Result<(), String> {
            self.opened_urls.lock().unwrap().push(url.to_string());
            Ok(())
        }

        fn copy_image_to_clipboard(&self, _image_path: &std::path::Path) -> Result<(), String> {
            Ok(())
        }

        fn search_browser_tabs(&self, _query: &str) -> Result<Vec<BrowserTab>, String> {
            Ok(Vec::new())
        }

        fn focus_browser_tab(&self, _target: &BrowserTabTarget) -> Result<(), String> {
            Ok(())
        }

        fn search_processes(&self, _query: &str) -> Result<Vec<ProcessMatch>, String> {
            Ok(Vec::new())
        }

        fn terminate_process(&self, _pid: u32) -> Result<(), String> {
            Ok(())
        }
    }

    struct StubGitHubCli {
        responses: Mutex<VecDeque<Result<Vec<GitHubPullRequest>, String>>>,
    }

    impl StubGitHubCli {
        fn new(responses: Vec<Result<Vec<GitHubPullRequest>, String>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
            }
        }
    }

    impl GitHubCli for StubGitHubCli {
        fn search_my_open_prs(&self) -> Result<Vec<GitHubPullRequest>, String> {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(Vec::new()))
        }
    }

    fn pr(number: u64, title: &str, repo: &str, draft: bool) -> GitHubPullRequest {
        GitHubPullRequest {
            title: title.into(),
            url: format!("https://github.com/{repo}/pull/{number}"),
            repository: GitHubRepository {
                name_with_owner: repo.into(),
            },
            number,
            updated_at: "2026-04-26T09:59:07Z".into(),
            is_draft: draft,
        }
    }

    #[test]
    fn provider_registers_command() {
        let provider = GitHubMyPrsProvider::with_cli(
            Arc::new(StubPlatform::default()),
            Arc::new(StubGitHubCli::new(vec![])),
        );

        let command = provider.commands().pop().unwrap();
        assert_eq!(command.id, CommandId::from(GITHUB_MY_PRS_COMMAND_ID));
    }

    #[test]
    fn provider_fetches_and_filters_pull_requests() {
        let provider = GitHubMyPrsProvider::with_cli(
            Arc::new(StubPlatform::default()),
            Arc::new(StubGitHubCli::new(vec![Ok(vec![
                pr(
                    1,
                    "Remove built-in hello command",
                    "alexandrelam/rayon",
                    false,
                ),
                pr(2, "Add metrics", "org/service", true),
            ])])),
        );
        let session = provider
            .start_interactive_session(&CommandId::from(GITHUB_MY_PRS_COMMAND_ID))
            .unwrap()
            .unwrap();
        let session = InteractiveSessionMetadata {
            session_id: "session-1".into(),
            ..session
        };

        let all_results = provider.search_interactive_session(&session, "").unwrap();
        assert_eq!(all_results.len(), 2);
        assert_eq!(
            all_results[0].subtitle.as_deref(),
            Some("alexandrelam/rayon #1 · open · updated 2026-04-26")
        );

        let filtered = provider
            .search_interactive_session(&session, "service")
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Add metrics");
    }

    #[test]
    fn provider_opens_selected_pull_request_and_completes() {
        let platform = Arc::new(StubPlatform::default());
        let provider = GitHubMyPrsProvider::with_cli(
            platform.clone(),
            Arc::new(StubGitHubCli::new(vec![Ok(vec![pr(
                1,
                "Remove built-in hello command",
                "alexandrelam/rayon",
                false,
            )])])),
        );
        let session = InteractiveSessionMetadata {
            session_id: "session-1".into(),
            command_id: CommandId::from(GITHUB_MY_PRS_COMMAND_ID),
            title: "My Pull Requests".into(),
            subtitle: Some("Open one of your authored pull requests".into()),
            input_placeholder: "Filter by title, repository, or number".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        };

        let result = provider
            .submit_interactive_session(
                &session,
                "",
                "https://github.com/alexandrelam/rayon/pull/1",
            )
            .unwrap();

        assert_eq!(
            platform.opened_urls.lock().unwrap().as_slice(),
            ["https://github.com/alexandrelam/rayon/pull/1"]
        );
        assert_eq!(
            result,
            InteractiveSessionSubmitOutcome::Completed(CommandExecutionResult {
                output: "opened alexandrelam/rayon#1".into(),
            })
        );
    }

    #[test]
    fn provider_surfaces_cli_errors() {
        let provider = GitHubMyPrsProvider::with_cli(
            Arc::new(StubPlatform::default()),
            Arc::new(StubGitHubCli::new(vec![Err(
                "GitHub CLI is not authenticated. Run gh auth login.".into(),
            )])),
        );
        let session = InteractiveSessionMetadata {
            session_id: "session-1".into(),
            command_id: CommandId::from(GITHUB_MY_PRS_COMMAND_ID),
            title: "My Pull Requests".into(),
            subtitle: Some("Open one of your authored pull requests".into()),
            input_placeholder: "Filter by title, repository, or number".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        };

        let error = provider
            .search_interactive_session(&session, "")
            .unwrap_err();
        assert_eq!(
            error,
            CommandError::ExecutionFailed(
                "GitHub CLI is not authenticated. Run gh auth login.".into()
            )
        );
    }
}
