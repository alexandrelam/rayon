use super::LauncherService;
use crate::commands::{CommandRegistry, APP_REINDEX_COMMAND_ID};
use crate::test_support::{
    build_launcher_service, CompletingProvider, StubPlatform, StubSearchIndex, TestProvider,
};
use crate::SearchIndexStats;
use crate::{CommandError, CommandProvider, InteractiveSessionSubmitOutcome};
use rayon_types::{
    BookmarkDefinition, BrowserTab, BrowserTabTarget, CommandDefinition, CommandExecutionRequest,
    CommandId, CommandInputMode, CommandInvocationResult, ImageAssetDefinition,
    InteractiveSessionCompletionBehavior, InteractiveSessionMetadata,
    InteractiveSessionQueryRequest, InteractiveSessionResult, InteractiveSessionSubmitRequest,
    InteractiveSessionSubmitResult, OpenWindow, OpenWindowTarget, SearchResultKind,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[allow(clippy::unwrap_used)]
fn launcher_with_platform(
    platform: Arc<StubPlatform>,
    search_results: Vec<String>,
) -> LauncherService {
    let mut registry = CommandRegistry::new();
    registry.register_provider(Arc::new(TestProvider)).unwrap();

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
            subtitle: Some("https://github.com".into()),
            owner_plugin_id: "user.bookmarks".into(),
            url: "https://github.com".into(),
            keywords: vec!["code".into()],
        }],
        Vec::new(),
        platform,
        index,
    )
}

#[allow(clippy::unwrap_used)]
#[test]
fn aggregate_search_uses_shared_index_ids() {
    let launcher = build_launcher_service(vec![
        String::from("echo"),
        String::from("app:macos:com.example.arc"),
        String::from("bookmark:github"),
    ]);

    let results = launcher.search("arc");

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].kind, SearchResultKind::Command);
    assert_eq!(results[1].kind, SearchResultKind::Application);
    assert_eq!(results[2].kind, SearchResultKind::Bookmark);
}

#[allow(clippy::unwrap_used)]
#[test]
fn aggregate_search_does_not_include_browser_tabs() {
    let platform = Arc::new(StubPlatform {
        apps: vec![rayon_types::InstalledApp {
            id: CommandId::from("app:macos:com.example.arc"),
            title: "Arc".into(),
            bundle_identifier: Some("com.example.arc".into()),
            path: "/Applications/Arc.app".into(),
        }],
        launched: Mutex::new(Vec::new()),
        opened_urls: Mutex::new(Vec::new()),
        copied_images: Mutex::new(Vec::new()),
        browser_tabs: Mutex::new(vec![BrowserTab {
            browser: "chrome".into(),
            window_id: "window-1".into(),
            window_index: 1,
            active_tab_index: 2,
            tab_index: 2,
            title: "Arc Docs".into(),
            url: "https://example.com/arc".into(),
        }]),
        focused_browser_tabs: Mutex::new(Vec::new()),
        open_windows: Mutex::new(Vec::new()),
        focused_open_windows: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let launcher = launcher_with_platform(platform, vec![String::from("echo")]);

    let results = launcher.search("arc");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, SearchResultKind::Command);
}

#[allow(clippy::unwrap_used)]
#[test]
fn execute_routes_app_ids_to_platform_launcher() {
    let launcher = build_launcher_service(vec![]);

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("app:macos:com.example.arc"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();

    assert_eq!(
        result,
        CommandInvocationResult::Completed {
            output: "opened Arc".into()
        }
    );
}

#[allow(clippy::unwrap_used)]
#[test]
fn execute_reindexes_shared_search_items() {
    let launcher = build_launcher_service(vec![]);

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from(APP_REINDEX_COMMAND_ID),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();

    assert_eq!(
        result,
        CommandInvocationResult::Completed {
            output: "reindexed 5 searchable items (0 skipped)".into()
        }
    );
}

#[allow(clippy::unwrap_used)]
#[test]
fn execute_routes_bookmark_ids_to_platform_url_opener() {
    let launcher = build_launcher_service(vec![]);

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("bookmark:github"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();

    assert_eq!(
        result,
        CommandInvocationResult::Completed {
            output: "opened GitHub".into()
        }
    );
}

#[allow(clippy::unwrap_used)]
#[test]
fn execute_routes_browser_tab_ids_to_platform_focus() {
    let platform = Arc::new(StubPlatform {
        apps: Vec::new(),
        launched: Mutex::new(Vec::new()),
        opened_urls: Mutex::new(Vec::new()),
        copied_images: Mutex::new(Vec::new()),
        browser_tabs: Mutex::new(vec![BrowserTab {
            browser: "chrome".into(),
            window_id: "window-1".into(),
            window_index: 1,
            active_tab_index: 4,
            tab_index: 4,
            title: "Issue 15".into(),
            url: "https://github.com/alexandrelam/rayon/issues/15".into(),
        }]),
        focused_browser_tabs: Mutex::new(Vec::new()),
        open_windows: Mutex::new(Vec::new()),
        focused_open_windows: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let launcher = launcher_with_platform(platform.clone(), vec![]);

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("browser-tab:chrome:window-1:4"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();

    assert_eq!(
        result,
        CommandInvocationResult::Completed {
            output: "focused Chrome tab".into()
        }
    );

    let focused = &platform.focused_browser_tabs.lock().unwrap()[0];
    assert_eq!(
        focused,
        &BrowserTabTarget {
            browser: "chrome".into(),
            window_id: "window-1".into(),
            tab_index: 4,
        }
    );
}

#[allow(clippy::unwrap_used)]
#[test]
fn execute_routes_open_window_ids_to_platform_focus() {
    let platform = Arc::new(StubPlatform {
        apps: Vec::new(),
        launched: Mutex::new(Vec::new()),
        opened_urls: Mutex::new(Vec::new()),
        copied_images: Mutex::new(Vec::new()),
        browser_tabs: Mutex::new(Vec::new()),
        focused_browser_tabs: Mutex::new(Vec::new()),
        open_windows: Mutex::new(vec![OpenWindow {
            application: "Arc".into(),
            pid: 4242,
            window_number: 777,
            bounds_x: 10,
            bounds_y: 20,
            bounds_width: 1440,
            bounds_height: 900,
            title: "Rayon".into(),
            is_frontmost: true,
        }]),
        focused_open_windows: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let launcher = launcher_with_platform(platform.clone(), vec![]);

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("open-window:4242:777:10:20:1440:900"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();

    assert_eq!(
        result,
        CommandInvocationResult::Completed {
            output: "focused window".into()
        }
    );

    let focused = &platform.focused_open_windows.lock().unwrap()[0];
    assert_eq!(
        focused,
        &OpenWindowTarget {
            pid: 4242,
            window_number: Some(777),
            bounds_x: 10,
            bounds_y: 20,
            bounds_width: 1440,
            bounds_height: 900,
        }
    );
}

#[allow(clippy::unwrap_used)]
#[test]
fn execute_starts_interactive_session_for_kill() {
    let launcher = build_launcher_service(vec![]);

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("kill"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();

    assert!(matches!(
        result,
        CommandInvocationResult::StartedSession { .. }
    ));
    let CommandInvocationResult::StartedSession { session } = result else {
        unreachable!();
    };
    assert_eq!(session.command_id, CommandId::from("kill"));
    assert_eq!(session.title, "Kill Process");
    assert_eq!(
        session.completion_behavior,
        InteractiveSessionCompletionBehavior::HideLauncher
    );
    assert!(session.is_loading);
    assert!(session.results.is_empty());
}

#[allow(clippy::unwrap_used)]
#[test]
fn interactive_session_search_and_submit_route_to_provider() {
    let launcher = build_launcher_service(vec![]);

    let session_result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("kill"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();
    assert!(matches!(
        session_result,
        CommandInvocationResult::StartedSession { .. }
    ));
    let CommandInvocationResult::StartedSession { session } = session_result else {
        unreachable!();
    };

    let searched = launcher
        .search_interactive_session(&InteractiveSessionQueryRequest {
            session_id: session.session_id.clone(),
            query: "8080".into(),
        })
        .unwrap();
    assert_eq!(searched.query, "8080");
    assert!(!searched.is_loading);
    assert_eq!(searched.results[0].id, "result:8080");
    assert_eq!(
        searched.completion_behavior,
        InteractiveSessionCompletionBehavior::HideLauncher
    );

    let submitted = launcher
        .submit_interactive_session(&InteractiveSessionSubmitRequest {
            session_id: session.session_id,
            query: "arc".into(),
            item_id: "1234".into(),
        })
        .unwrap();
    let session = match submitted {
        InteractiveSessionSubmitResult::UpdatedSession { session } => session,
        InteractiveSessionSubmitResult::Completed { .. } => unreachable!(),
    };
    assert_eq!(session.message, Some("terminated 1234".into()));
    assert_eq!(session.results[0].id, "refresh:arc");
}

#[allow(clippy::unwrap_used)]
#[test]
fn completed_interactive_submit_removes_active_session() {
    let mut registry = CommandRegistry::new();
    registry
        .register_provider(Arc::new(CompletingProvider))
        .unwrap();

    let platform = Arc::new(StubPlatform {
        apps: Vec::new(),
        launched: Mutex::new(Vec::new()),
        opened_urls: Mutex::new(Vec::new()),
        copied_images: Mutex::new(Vec::new()),
        browser_tabs: Mutex::new(Vec::new()),
        focused_browser_tabs: Mutex::new(Vec::new()),
        open_windows: Mutex::new(Vec::new()),
        focused_open_windows: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let index = Arc::new(StubSearchIndex {
        configured: true,
        search_results: Vec::new(),
        stats: SearchIndexStats {
            discovered_count: 0,
            indexed_count: 0,
            skipped_count: 0,
        },
        last_documents: Mutex::new(Vec::new()),
    });
    let launcher = LauncherService::new(registry, Vec::new(), Vec::new(), platform, index);

    let session_result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("github.my-prs"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();
    let session = match session_result {
        CommandInvocationResult::StartedSession { session } => session,
        CommandInvocationResult::Completed { .. } => unreachable!(),
    };

    let result = launcher
        .submit_interactive_session(&InteractiveSessionSubmitRequest {
            session_id: session.session_id.clone(),
            query: String::new(),
            item_id: "https://github.com/example/repo/pull/1".into(),
        })
        .unwrap();
    assert_eq!(
        result,
        InteractiveSessionSubmitResult::Completed {
            output: "opened example/repo#1".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncher,
        }
    );

    let error = launcher
        .search_interactive_session(&InteractiveSessionQueryRequest {
            session_id: session.session_id,
            query: String::new(),
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "unknown interactive session: session-1");
}

#[allow(clippy::unwrap_used)]
#[test]
fn completed_interactive_submit_calls_provider_cleanup() {
    struct CleanupTrackingProvider {
        ended_sessions: Arc<Mutex<Vec<String>>>,
    }

    impl CommandProvider for CleanupTrackingProvider {
        fn commands(&self) -> Vec<CommandDefinition> {
            vec![CommandDefinition {
                id: CommandId::from("cleanup.test"),
                title: "Cleanup Test".into(),
                subtitle: None,
                owner_plugin_id: "builtin.test".into(),
                keywords: Vec::new(),
                close_launcher_on_success: false,
                input_mode: CommandInputMode::Structured,
                arguments: Vec::new(),
            }]
        }

        fn execute(
            &self,
            request: &CommandExecutionRequest,
        ) -> Result<rayon_types::CommandExecutionResult, CommandError> {
            Err(CommandError::UnknownCommand(request.command_id.clone()))
        }

        fn start_interactive_session(
            &self,
            command_id: &CommandId,
        ) -> Result<Option<InteractiveSessionMetadata>, CommandError> {
            if command_id.as_str() != "cleanup.test" {
                return Ok(None);
            }

            Ok(Some(InteractiveSessionMetadata {
                session_id: String::new(),
                command_id: command_id.clone(),
                title: "Cleanup Test".into(),
                subtitle: None,
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
                id: "done".into(),
                title: "Done".into(),
                subtitle: None,
            }])
        }

        fn submit_interactive_session(
            &self,
            _session: &InteractiveSessionMetadata,
            _query: &str,
            _item_id: &str,
        ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
            Ok(InteractiveSessionSubmitOutcome::Completed(
                rayon_types::CommandExecutionResult {
                    output: "done".into(),
                },
            ))
        }

        fn end_interactive_session(&self, session: &InteractiveSessionMetadata) {
            self.ended_sessions
                .lock()
                .unwrap()
                .push(session.session_id.clone());
        }
    }

    let ended_sessions = Arc::new(Mutex::new(Vec::new()));
    let mut registry = CommandRegistry::new();
    registry
        .register_provider(Arc::new(CleanupTrackingProvider {
            ended_sessions: ended_sessions.clone(),
        }))
        .unwrap();

    let platform = Arc::new(StubPlatform {
        apps: Vec::new(),
        launched: Mutex::new(Vec::new()),
        opened_urls: Mutex::new(Vec::new()),
        copied_images: Mutex::new(Vec::new()),
        browser_tabs: Mutex::new(Vec::new()),
        focused_browser_tabs: Mutex::new(Vec::new()),
        open_windows: Mutex::new(Vec::new()),
        focused_open_windows: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let index = Arc::new(StubSearchIndex {
        configured: true,
        search_results: Vec::new(),
        stats: SearchIndexStats {
            discovered_count: 0,
            indexed_count: 0,
            skipped_count: 0,
        },
        last_documents: Mutex::new(Vec::new()),
    });
    let launcher = LauncherService::new(registry, Vec::new(), Vec::new(), platform, index);

    let session = match launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("cleanup.test"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap()
    {
        CommandInvocationResult::StartedSession { session } => session,
        CommandInvocationResult::Completed { .. } => unreachable!(),
    };

    let _ = launcher
        .submit_interactive_session(&InteractiveSessionSubmitRequest {
            session_id: session.session_id.clone(),
            query: String::new(),
            item_id: "done".into(),
        })
        .unwrap();

    assert_eq!(&*ended_sessions.lock().unwrap(), &[session.session_id]);
}

#[allow(clippy::unwrap_used)]
#[test]
fn startup_reindexes_commands_and_apps() {
    let mut registry = CommandRegistry::new();
    registry.register_provider(Arc::new(TestProvider)).unwrap();

    let platform = Arc::new(StubPlatform {
        apps: vec![rayon_types::InstalledApp {
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
        open_windows: Mutex::new(Vec::new()),
        focused_open_windows: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let index = Arc::new(StubSearchIndex {
        configured: true,
        search_results: Vec::new(),
        stats: SearchIndexStats {
            discovered_count: 0,
            indexed_count: 5,
            skipped_count: 0,
        },
        last_documents: Mutex::new(Vec::new()),
    });

    let _launcher = LauncherService::new(
        registry,
        vec![BookmarkDefinition {
            id: CommandId::from("bookmark:github"),
            title: "GitHub".into(),
            subtitle: Some("Code hosting".into()),
            owner_plugin_id: "user.links".into(),
            url: "https://github.com".into(),
            keywords: vec!["git".into()],
        }],
        Vec::new(),
        platform,
        index.clone(),
    );
    let documents = index.last_documents.lock().unwrap();

    assert_eq!(documents.len(), 4);
    assert!(documents
        .iter()
        .any(|document| document.id == CommandId::from("echo")));
    assert!(documents
        .iter()
        .any(|document| document.id == CommandId::from("app:macos:com.example.arc")));
    assert!(documents
        .iter()
        .any(|document| document.id == CommandId::from("bookmark:github")));
    assert!(documents
        .iter()
        .any(|document| document.id == CommandId::from("kill")));
}

#[allow(clippy::unwrap_used)]
#[test]
fn command_search_results_include_close_launcher_flag() {
    let launcher = build_launcher_service(vec![String::from("echo")]);

    let results = launcher.search("echo");
    let command = results
        .into_iter()
        .find(|result| result.id == CommandId::from("echo"))
        .unwrap();

    assert!(!command.close_launcher_on_success);
}

#[allow(clippy::unwrap_used)]
#[test]
fn aggregate_search_includes_images() {
    let platform = Arc::new(StubPlatform {
        apps: Vec::new(),
        launched: Mutex::new(Vec::new()),
        opened_urls: Mutex::new(Vec::new()),
        copied_images: Mutex::new(Vec::new()),
        browser_tabs: Mutex::new(Vec::new()),
        focused_browser_tabs: Mutex::new(Vec::new()),
        open_windows: Mutex::new(Vec::new()),
        focused_open_windows: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let launcher = LauncherService::new(
        CommandRegistry::new(),
        Vec::new(),
        vec![ImageAssetDefinition {
            id: CommandId::from("image-asset:logos/brand.png"),
            title: "brand.png".into(),
            relative_path: "logos/brand.png".into(),
            path: "/tmp/logos/brand.png".into(),
        }],
        platform,
        Arc::new(StubSearchIndex {
            configured: true,
            search_results: vec![String::from("image-asset:logos/brand.png")],
            stats: SearchIndexStats {
                discovered_count: 0,
                indexed_count: 1,
                skipped_count: 0,
            },
            last_documents: Mutex::new(Vec::new()),
        }),
    );

    let results = launcher.search("brand");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, SearchResultKind::Image);
    assert!(results[0].close_launcher_on_success);
}

#[allow(clippy::unwrap_used)]
#[test]
fn execute_routes_image_ids_to_clipboard_copy() {
    let platform = Arc::new(StubPlatform {
        apps: Vec::new(),
        launched: Mutex::new(Vec::new()),
        opened_urls: Mutex::new(Vec::new()),
        copied_images: Mutex::new(Vec::new()),
        browser_tabs: Mutex::new(Vec::new()),
        focused_browser_tabs: Mutex::new(Vec::new()),
        open_windows: Mutex::new(Vec::new()),
        focused_open_windows: Mutex::new(Vec::new()),
        process_search_results: Mutex::new(Vec::new()),
        terminated_pids: Mutex::new(Vec::new()),
    });
    let launcher = LauncherService::new(
        CommandRegistry::new(),
        Vec::new(),
        vec![ImageAssetDefinition {
            id: CommandId::from("image-asset:logos/brand.png"),
            title: "brand.png".into(),
            relative_path: "logos/brand.png".into(),
            path: "/tmp/logos/brand.png".into(),
        }],
        platform.clone(),
        Arc::new(StubSearchIndex {
            configured: true,
            search_results: Vec::new(),
            stats: SearchIndexStats {
                discovered_count: 0,
                indexed_count: 0,
                skipped_count: 0,
            },
            last_documents: Mutex::new(Vec::new()),
        }),
    );

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("image-asset:logos/brand.png"),
            argv: Vec::new(),
            arguments: HashMap::new(),
        })
        .unwrap();

    assert_eq!(
        result,
        CommandInvocationResult::Completed {
            output: "copied brand.png".into()
        }
    );
    assert_eq!(
        &*platform.copied_images.lock().unwrap(),
        &[String::from("/tmp/logos/brand.png")]
    );
}
