use super::service::LauncherService;
use super::state::read_app_catalog;
use crate::SearchIndexStats;
use rayon_types::SearchResult;

const SEARCH_LIMIT: usize = 20;

impl LauncherService {
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
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

        let mut results = Vec::new();
        for item_id in item_ids {
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
}
