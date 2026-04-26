use super::LauncherService;
use crate::commands::{CommandRegistry, APP_REINDEX_COMMAND_ID};
use crate::test_support::{
    build_launcher_service, CompletingProvider, StubPlatform, StubSearchIndex, TestProvider,
};
use rayon_db::SearchIndexStats;
use rayon_types::{
    BookmarkDefinition, CommandExecutionRequest, CommandId, CommandInvocationResult,
    InteractiveSessionQueryRequest, InteractiveSessionSubmitRequest,
    InteractiveSessionSubmitResult, SearchResultKind,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
fn execute_routes_app_ids_to_platform_launcher() {
    let launcher = build_launcher_service(vec![]);

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("app:macos:com.example.arc"),
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
fn execute_starts_interactive_session_for_kill() {
    let launcher = build_launcher_service(vec![]);

    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("kill"),
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
    let launcher = LauncherService::new(registry, Vec::new(), platform, index);

    let session_result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: CommandId::from("github.my-prs"),
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
            output: "opened example/repo#1".into()
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
