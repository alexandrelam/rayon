use rayon_core::{AppPlatform, CommandError, CommandProvider, InteractiveSessionSubmitOutcome};
use rayon_types::{
    CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId,
    CommandInputMode, InteractiveSessionCompletionBehavior, InteractiveSessionMetadata,
    InteractiveSessionResult,
};
use serde::Deserialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

const GITHUB_MY_PRS_COMMAND_ID: &str = "github.my-prs";

pub struct GitHubMyPrsProvider {
    platform: Arc<dyn AppPlatform>,
    cli: Arc<dyn GitHubCli>,
    cache: Arc<Mutex<GitHubPrCache>>,
    refresh_scheduler: Arc<dyn RefreshScheduler>,
}

impl GitHubMyPrsProvider {
    pub fn new(platform: Arc<dyn AppPlatform>) -> Self {
        Self::with_dependencies(
            platform,
            Arc::new(SystemGitHubCli),
            Arc::new(ThreadRefreshScheduler),
        )
    }

    #[cfg(test)]
    fn with_cli(platform: Arc<dyn AppPlatform>, cli: Arc<dyn GitHubCli>) -> Self {
        Self::with_dependencies(platform, cli, Arc::new(ThreadRefreshScheduler))
    }

    fn with_dependencies(
        platform: Arc<dyn AppPlatform>,
        cli: Arc<dyn GitHubCli>,
        refresh_scheduler: Arc<dyn RefreshScheduler>,
    ) -> Self {
        Self {
            platform,
            cli,
            cache: Arc::new(Mutex::new(GitHubPrCache::default())),
            refresh_scheduler,
        }
    }

    fn cached_prs(&self) -> Result<Option<Vec<GitHubPullRequest>>, CommandError> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| CommandError::ExecutionFailed("GitHub PR cache lock poisoned".into()))?;
        Ok(cache.prs.clone())
    }

    fn session_prs(
        &self,
        _session: &InteractiveSessionMetadata,
    ) -> Result<Vec<GitHubPullRequest>, CommandError> {
        if let Some(prs) = self.cached_prs()? {
            self.schedule_refresh_if_needed()?;
            return Ok(prs);
        }

        let prs = self
            .cli
            .search_my_open_prs()
            .map_err(CommandError::ExecutionFailed)?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| CommandError::ExecutionFailed("GitHub PR cache lock poisoned".into()))?;
        cache.prs = Some(prs.clone());
        cache.last_refresh_error = None;
        Ok(prs)
    }

    fn schedule_refresh_if_needed(&self) -> Result<(), CommandError> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| CommandError::ExecutionFailed("GitHub PR cache lock poisoned".into()))?;
        if cache.refresh_in_flight {
            return Ok(());
        }

        cache.refresh_in_flight = true;
        drop(cache);

        let cli = Arc::clone(&self.cli);
        let cache = Arc::clone(&self.cache);
        self.refresh_scheduler.schedule(Box::new(move || {
            let refresh_result = cli.search_my_open_prs();
            match cache.lock() {
                Ok(mut cache) => {
                    if let Ok(prs) = refresh_result {
                        cache.prs = Some(prs);
                        cache.last_refresh_error = None;
                    } else if let Err(error) = refresh_result {
                        eprintln!("github pr refresh failed: {error}");
                        cache.last_refresh_error = Some(error);
                    }
                    cache.refresh_in_flight = false;
                }
                Err(_) => {
                    if let Err(error) = refresh_result {
                        eprintln!("github pr refresh failed after cache lock poison: {error}");
                    }
                }
            }
        }));
        Ok(())
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

    fn end_interactive_session(&self, _session: &InteractiveSessionMetadata) {}
}

trait GitHubCli: Send + Sync {
    fn search_my_open_prs(&self) -> Result<Vec<GitHubPullRequest>, String>;
}

trait RefreshScheduler: Send + Sync {
    fn schedule(&self, task: Box<dyn FnOnce() + Send>);
}

struct ThreadRefreshScheduler;

impl RefreshScheduler for ThreadRefreshScheduler {
    fn schedule(&self, task: Box<dyn FnOnce() + Send>) {
        let _ = std::thread::Builder::new()
            .name("github-pr-refresh".into())
            .spawn(task);
    }
}

#[derive(Default)]
struct GitHubPrCache {
    prs: Option<Vec<GitHubPullRequest>>,
    refresh_in_flight: bool,
    last_refresh_error: Option<String>,
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
    let gh = resolve_gh_path()?;
    Command::new(&gh).args(args).output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "GitHub CLI (gh) was resolved to {} but could not be executed.",
                gh.display()
            )
        } else {
            format!("failed to run gh: {error}")
        }
    })
}

fn resolve_gh_path() -> Result<PathBuf, String> {
    if let Some(path) = find_program_in_current_path("gh") {
        return Ok(path);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(path) = find_program_in_paths("gh", macos_cli_search_paths()) {
            return Ok(path);
        }

        if let Some(path) = resolve_program_via_login_shell("gh") {
            return Ok(path);
        }
    }

    Err("GitHub CLI (gh) is not installed or not available to Rayon. Install it with `brew install gh`, or make sure the app can access the directory that contains `gh`.".into())
}

fn find_program_in_current_path(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| find_program_in_path_list(program, &paths))
}

fn find_program_in_path_list(program: &str, paths: &OsString) -> Option<PathBuf> {
    std::env::split_paths(paths)
        .map(|directory| directory.join(program))
        .find(|candidate| is_executable_file(candidate))
}

fn find_program_in_paths(
    program: &str,
    directories: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    directories
        .into_iter()
        .map(|directory| directory.join(program))
        .find(|candidate| is_executable_file(candidate))
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(target_os = "macos")]
fn macos_cli_search_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/local/bin"),
        PathBuf::from("/usr/bin"),
    ]
}

#[cfg(target_os = "macos")]
fn resolve_program_via_login_shell(program: &str) -> Option<PathBuf> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let output = Command::new(shell)
        .args(["-l", "-c", &format!("command -v {program}")])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let candidate = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if candidate.is_empty() {
        return None;
    }

    let path = PathBuf::from(candidate);
    is_executable_file(&path).then_some(path)
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
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Condvar;

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
        call_count: AtomicUsize,
    }

    impl StubGitHubCli {
        fn new(responses: Vec<Result<Vec<GitHubPullRequest>, String>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                call_count: AtomicUsize::new(0),
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl GitHubCli for StubGitHubCli {
        fn search_my_open_prs(&self) -> Result<Vec<GitHubPullRequest>, String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(Vec::new()))
        }
    }

    #[derive(Default)]
    struct ManualRefreshScheduler {
        tasks: Mutex<VecDeque<Box<dyn FnOnce() + Send>>>,
    }

    impl ManualRefreshScheduler {
        fn pending_count(&self) -> usize {
            self.tasks.lock().unwrap().len()
        }

        fn run_next(&self) {
            if let Some(task) = self.tasks.lock().unwrap().pop_front() {
                task();
            }
        }
    }

    impl RefreshScheduler for ManualRefreshScheduler {
        fn schedule(&self, task: Box<dyn FnOnce() + Send>) {
            self.tasks.lock().unwrap().push_back(task);
        }
    }

    struct BlockingRefreshScheduler {
        inner: Arc<ManualRefreshScheduler>,
        started: Arc<(Mutex<usize>, Condvar)>,
    }

    impl BlockingRefreshScheduler {
        fn new(inner: Arc<ManualRefreshScheduler>) -> Self {
            Self {
                inner,
                started: Arc::new((Mutex::new(0), Condvar::new())),
            }
        }

        fn wait_for_schedule_count(&self, expected: usize) {
            let (lock, condvar) = &*self.started;
            let mut count = lock.lock().unwrap();
            while *count < expected {
                count = condvar.wait(count).unwrap();
            }
        }
    }

    impl RefreshScheduler for BlockingRefreshScheduler {
        fn schedule(&self, task: Box<dyn FnOnce() + Send>) {
            self.inner.schedule(task);
            let (lock, condvar) = &*self.started;
            let mut count = lock.lock().unwrap();
            *count += 1;
            condvar.notify_all();
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
    fn provider_reuses_cached_results_across_sessions() {
        let cli = Arc::new(StubGitHubCli::new(vec![Ok(vec![pr(
            1,
            "Remove built-in hello command",
            "alexandrelam/rayon",
            false,
        )])]));
        let scheduler = Arc::new(ManualRefreshScheduler::default());
        let provider = GitHubMyPrsProvider::with_dependencies(
            Arc::new(StubPlatform::default()),
            cli.clone(),
            scheduler.clone(),
        );

        let first_session = InteractiveSessionMetadata {
            session_id: "session-1".into(),
            command_id: CommandId::from(GITHUB_MY_PRS_COMMAND_ID),
            title: "My Pull Requests".into(),
            subtitle: Some("Open one of your authored pull requests".into()),
            input_placeholder: "Filter by title, repository, or number".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        };
        let second_session = InteractiveSessionMetadata {
            session_id: "session-2".into(),
            ..first_session.clone()
        };

        let first_results = provider
            .search_interactive_session(&first_session, "")
            .unwrap();
        let second_results = provider
            .search_interactive_session(&second_session, "")
            .unwrap();

        assert_eq!(first_results, second_results);
        assert_eq!(cli.call_count(), 1);
        assert_eq!(scheduler.pending_count(), 1);
    }

    #[test]
    fn provider_refreshes_cached_results_in_background_for_next_open() {
        let cli = Arc::new(StubGitHubCli::new(vec![
            Ok(vec![pr(1, "Old title", "alexandrelam/rayon", false)]),
            Ok(vec![pr(2, "New title", "alexandrelam/rayon", false)]),
        ]));
        let scheduler = Arc::new(ManualRefreshScheduler::default());
        let provider = GitHubMyPrsProvider::with_dependencies(
            Arc::new(StubPlatform::default()),
            cli,
            scheduler.clone(),
        );

        let first_session = InteractiveSessionMetadata {
            session_id: "session-1".into(),
            command_id: CommandId::from(GITHUB_MY_PRS_COMMAND_ID),
            title: "My Pull Requests".into(),
            subtitle: Some("Open one of your authored pull requests".into()),
            input_placeholder: "Filter by title, repository, or number".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        };
        let second_session = InteractiveSessionMetadata {
            session_id: "session-2".into(),
            ..first_session.clone()
        };
        let third_session = InteractiveSessionMetadata {
            session_id: "session-3".into(),
            ..first_session.clone()
        };

        let first_results = provider
            .search_interactive_session(&first_session, "")
            .unwrap();
        let second_results = provider
            .search_interactive_session(&second_session, "")
            .unwrap();

        assert_eq!(first_results[0].title, "Old title");
        assert_eq!(second_results[0].title, "Old title");
        assert_eq!(scheduler.pending_count(), 1);

        scheduler.run_next();

        let third_results = provider
            .search_interactive_session(&third_session, "")
            .unwrap();
        assert_eq!(third_results[0].title, "New title");
    }

    #[test]
    fn provider_keeps_stale_results_when_background_refresh_fails() {
        let cli = Arc::new(StubGitHubCli::new(vec![
            Ok(vec![pr(1, "Stable title", "alexandrelam/rayon", false)]),
            Err("refresh failed".into()),
        ]));
        let scheduler = Arc::new(ManualRefreshScheduler::default());
        let provider = GitHubMyPrsProvider::with_dependencies(
            Arc::new(StubPlatform::default()),
            cli,
            scheduler.clone(),
        );

        let first_session = InteractiveSessionMetadata {
            session_id: "session-1".into(),
            command_id: CommandId::from(GITHUB_MY_PRS_COMMAND_ID),
            title: "My Pull Requests".into(),
            subtitle: Some("Open one of your authored pull requests".into()),
            input_placeholder: "Filter by title, repository, or number".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        };
        let second_session = InteractiveSessionMetadata {
            session_id: "session-2".into(),
            ..first_session.clone()
        };
        let third_session = InteractiveSessionMetadata {
            session_id: "session-3".into(),
            ..first_session.clone()
        };

        provider
            .search_interactive_session(&first_session, "")
            .unwrap();
        provider
            .search_interactive_session(&second_session, "")
            .unwrap();
        scheduler.run_next();

        let third_results = provider
            .search_interactive_session(&third_session, "")
            .unwrap();
        assert_eq!(third_results[0].title, "Stable title");
    }

    #[test]
    fn provider_schedules_only_one_background_refresh_at_a_time() {
        let cli = Arc::new(StubGitHubCli::new(vec![
            Ok(vec![pr(1, "Stable title", "alexandrelam/rayon", false)]),
            Ok(vec![pr(2, "Updated title", "alexandrelam/rayon", false)]),
        ]));
        let manual_scheduler = Arc::new(ManualRefreshScheduler::default());
        let scheduler = Arc::new(BlockingRefreshScheduler::new(manual_scheduler.clone()));
        let provider = Arc::new(GitHubMyPrsProvider::with_dependencies(
            Arc::new(StubPlatform::default()),
            cli.clone(),
            scheduler.clone(),
        ));

        let base_session = InteractiveSessionMetadata {
            session_id: "session-1".into(),
            command_id: CommandId::from(GITHUB_MY_PRS_COMMAND_ID),
            title: "My Pull Requests".into(),
            subtitle: Some("Open one of your authored pull requests".into()),
            input_placeholder: "Filter by title, repository, or number".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        };

        provider
            .search_interactive_session(&base_session, "")
            .unwrap();

        let provider_a = provider.clone();
        let session_a = InteractiveSessionMetadata {
            session_id: "session-2".into(),
            ..base_session.clone()
        };
        let handle_a = std::thread::spawn(move || {
            provider_a
                .search_interactive_session(&session_a, "")
                .unwrap();
        });

        scheduler.wait_for_schedule_count(1);

        let provider_b = provider.clone();
        let session_b = InteractiveSessionMetadata {
            session_id: "session-3".into(),
            ..base_session.clone()
        };
        let handle_b = std::thread::spawn(move || {
            provider_b
                .search_interactive_session(&session_b, "")
                .unwrap();
        });

        handle_a.join().unwrap();
        handle_b.join().unwrap();

        assert_eq!(manual_scheduler.pending_count(), 1);
        assert_eq!(cli.call_count(), 1);
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

    #[test]
    fn finds_program_in_path_list() {
        let tempdir = std::env::temp_dir().join(format!("rayon-gh-test-{}", std::process::id()));
        fs::create_dir_all(&tempdir).unwrap();
        let gh_path = tempdir.join("gh");
        fs::write(&gh_path, "#!/bin/sh\n").unwrap();

        let resolved = find_program_in_path_list("gh", &OsString::from(tempdir.as_os_str()));

        assert_eq!(resolved, Some(gh_path));

        fs::remove_file(tempdir.join("gh")).unwrap();
        fs::remove_dir(tempdir).unwrap();
    }

    #[test]
    fn skips_missing_program_in_path_list() {
        let tempdir =
            std::env::temp_dir().join(format!("rayon-gh-missing-test-{}", std::process::id()));
        fs::create_dir_all(&tempdir).unwrap();

        let resolved = find_program_in_path_list("gh", &OsString::from(tempdir.as_os_str()));

        assert_eq!(resolved, None);

        fs::remove_dir(tempdir).unwrap();
    }
}
