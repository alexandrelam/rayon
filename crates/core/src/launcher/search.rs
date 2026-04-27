use super::service::LauncherService;
use super::state::read_app_catalog;
use rayon_db::SearchIndexStats;
use rayon_types::{CommandInputMode, SearchResult, SearchResultKind};

const SEARCH_LIMIT: usize = 20;

impl LauncherService {
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let mut results = self.browser_tab_results(query);
        let item_ids = match self.search_index.search_item_ids(query, SEARCH_LIMIT) {
            Ok(item_ids) => item_ids,
            Err(error) => {
                eprintln!("search failed: {error}");
                Vec::new()
            }
        };

        let mut search_results = self.registry.search_results_by_id();
        let app_results = read_app_catalog(&self.app_catalog).search_results_by_id();
        search_results.extend(app_results);
        search_results.extend(self.bookmark_catalog.search_results_by_id());

        for item_id in item_ids {
            if results.len() >= SEARCH_LIMIT {
                break;
            }

            if let Some(result) = search_results.get(&item_id).cloned() {
                results.push(result);
            }
        }

        results
    }

    pub fn search_enabled(&self) -> bool {
        self.search_index.is_configured()
    }

    pub(super) fn reindex_search(&self) -> Result<SearchIndexStats, String> {
        let mut documents = self.registry.searchable_documents();
        let app_documents = read_app_catalog(&self.app_catalog).searchable_documents();
        documents.extend(app_documents);
        documents.extend(self.bookmark_catalog.searchable_documents());
        self.search_index.replace_items(&documents)
    }

    fn browser_tab_results(&self, query: &str) -> Vec<SearchResult> {
        match self.platform.search_browser_tabs(query) {
            Ok(tabs) => tabs
                .into_iter()
                .take(SEARCH_LIMIT)
                .map(|tab| {
                    let subtitle = tab.subtitle();
                    SearchResult {
                        id: tab.command_id(),
                        title: tab.title,
                        subtitle: Some(subtitle),
                        icon_path: None,
                        kind: SearchResultKind::BrowserTab,
                        owner_plugin_id: None,
                        keywords: Vec::new(),
                        starts_interactive_session: false,
                        close_launcher_on_success: true,
                        input_mode: CommandInputMode::Structured,
                        arguments: Vec::new(),
                    }
                })
                .collect(),
            Err(error) => {
                eprintln!("browser tab search failed: {error}");
                Vec::new()
            }
        }
    }
}
