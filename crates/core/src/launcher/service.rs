use super::state::ActiveInteractiveSession;
use crate::catalog::{AppCatalog, AppPlatform, BookmarkCatalog, SearchIndex};
use crate::commands::CommandRegistry;
use rayon_types::BookmarkDefinition;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

pub struct LauncherService {
    pub(super) registry: CommandRegistry,
    pub(super) platform: Arc<dyn AppPlatform>,
    pub(super) search_index: Arc<dyn SearchIndex>,
    pub(super) app_catalog: RwLock<AppCatalog>,
    pub(super) bookmark_catalog: BookmarkCatalog,
    pub(super) interactive_sessions: RwLock<HashMap<String, ActiveInteractiveSession>>,
    pub(super) next_session_id: AtomicU64,
}

impl LauncherService {
    pub fn new(
        registry: CommandRegistry,
        bookmarks: Vec<BookmarkDefinition>,
        platform: Arc<dyn AppPlatform>,
        search_index: Arc<dyn SearchIndex>,
    ) -> Self {
        let app_catalog = match platform.discover_apps() {
            Ok(apps) => AppCatalog::from_apps(apps),
            Err(error) => {
                eprintln!("failed to discover apps on startup: {error}");
                AppCatalog::default()
            }
        };

        let service = Self {
            registry,
            platform,
            search_index,
            app_catalog: RwLock::new(app_catalog),
            bookmark_catalog: BookmarkCatalog::from_bookmarks(bookmarks),
            interactive_sessions: RwLock::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
        };

        if let Err(error) = service.reindex_search() {
            eprintln!("failed to build search index on startup: {error}");
        }

        service
    }
}
