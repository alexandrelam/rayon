use rayon_types::{
    BookmarkDefinition, BrowserTab, BrowserTabTarget, CommandId, CommandInputMode, InstalledApp,
    ProcessMatch, SearchIndexStats, SearchResult, SearchResultKind, SearchableItemDocument,
};
use std::collections::HashMap;

pub trait AppPlatform: Send + Sync {
    fn discover_apps(&self) -> Result<Vec<InstalledApp>, String>;
    fn launch_app(&self, app: &InstalledApp) -> Result<(), String>;
    fn open_url(&self, url: &str) -> Result<(), String>;
    fn search_browser_tabs(&self, query: &str) -> Result<Vec<BrowserTab>, String>;
    fn focus_browser_tab(&self, target: &BrowserTabTarget) -> Result<(), String>;
    fn search_processes(&self, query: &str) -> Result<Vec<ProcessMatch>, String>;
    fn terminate_process(&self, pid: u32) -> Result<(), String>;
}

pub trait SearchIndex: Send + Sync {
    fn is_configured(&self) -> bool;
    fn search_item_ids(&self, query: &str, limit: usize) -> Result<Vec<String>, String>;
    fn replace_items(&self, items: &[SearchableItemDocument]) -> Result<SearchIndexStats, String>;
}

#[derive(Default)]
pub(crate) struct AppCatalog {
    by_id: HashMap<String, InstalledApp>,
}

impl AppCatalog {
    pub(crate) fn from_apps(apps: Vec<InstalledApp>) -> Self {
        let mut by_id = HashMap::new();
        for app in apps {
            by_id.insert(app.id.to_string(), app);
        }

        Self { by_id }
    }

    pub(crate) fn get(&self, app_id: &CommandId) -> Option<&InstalledApp> {
        self.by_id.get(app_id.as_str())
    }

    pub(crate) fn searchable_documents(&self) -> Vec<SearchableItemDocument> {
        self.by_id
            .values()
            .map(|app| SearchableItemDocument {
                id: app.id.clone(),
                kind: SearchResultKind::Application,
                title: app.title.clone(),
                subtitle: Some(app.subtitle()),
                owner_plugin_id: None,
                search_text: app.search_text(),
            })
            .collect()
    }

    pub(crate) fn search_results_by_id(&self) -> HashMap<String, SearchResult> {
        self.by_id
            .values()
            .map(|app| {
                (
                    app.id.to_string(),
                    SearchResult {
                        id: app.id.clone(),
                        title: app.title.clone(),
                        subtitle: Some(app.subtitle()),
                        icon_path: None,
                        kind: SearchResultKind::Application,
                        owner_plugin_id: None,
                        keywords: Vec::new(),
                        starts_interactive_session: false,
                        close_launcher_on_success: false,
                        input_mode: CommandInputMode::Structured,
                        arguments: Vec::new(),
                    },
                )
            })
            .collect()
    }
}

#[derive(Default)]
pub(crate) struct BookmarkCatalog {
    by_id: HashMap<String, BookmarkDefinition>,
}

impl BookmarkCatalog {
    pub(crate) fn from_bookmarks(bookmarks: Vec<BookmarkDefinition>) -> Self {
        let mut by_id = HashMap::new();
        for bookmark in bookmarks {
            by_id.insert(bookmark.id.to_string(), bookmark);
        }

        Self { by_id }
    }

    pub(crate) fn get(&self, bookmark_id: &CommandId) -> Option<&BookmarkDefinition> {
        self.by_id.get(bookmark_id.as_str())
    }

    pub(crate) fn search_results_by_id(&self) -> HashMap<String, SearchResult> {
        self.by_id
            .values()
            .map(|bookmark| {
                (
                    bookmark.id.to_string(),
                    SearchResult {
                        id: bookmark.id.clone(),
                        title: bookmark.title.clone(),
                        subtitle: bookmark.subtitle.clone(),
                        icon_path: None,
                        kind: SearchResultKind::Bookmark,
                        owner_plugin_id: Some(bookmark.owner_plugin_id.clone()),
                        keywords: Vec::new(),
                        starts_interactive_session: false,
                        close_launcher_on_success: false,
                        input_mode: CommandInputMode::Structured,
                        arguments: Vec::new(),
                    },
                )
            })
            .collect()
    }

    pub(crate) fn searchable_documents(&self) -> Vec<SearchableItemDocument> {
        self.by_id
            .values()
            .map(|bookmark| SearchableItemDocument {
                id: bookmark.id.clone(),
                kind: SearchResultKind::Bookmark,
                title: bookmark.title.clone(),
                subtitle: bookmark.subtitle.clone(),
                owner_plugin_id: Some(bookmark.owner_plugin_id.clone()),
                search_text: bookmark_search_text(bookmark),
            })
            .collect()
    }
}

fn bookmark_search_text(definition: &BookmarkDefinition) -> String {
    let mut parts = vec![
        definition.id.to_string(),
        definition.title.clone(),
        definition.owner_plugin_id.clone(),
        definition.url.clone(),
    ];
    if let Some(subtitle) = &definition.subtitle {
        parts.push(subtitle.clone());
    }
    parts.extend(definition.keywords.clone());
    parts.join(" ")
}
