use rayon_core::{AppPlatform, SearchIndex, SearchIndexStats};
use rayon_db::TantivySearchIndex;
use rayon_platform::MacOsAppManager;
use rayon_types::{
    BrowserTab, BrowserTabTarget, InstalledApp, ProcessMatch, SearchableItemDocument,
};

pub(super) struct DesktopPlatform {
    inner: MacOsAppManager,
}

impl DesktopPlatform {
    pub(super) fn new() -> Self {
        Self {
            inner: MacOsAppManager,
        }
    }
}

impl AppPlatform for DesktopPlatform {
    fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
        self.inner.discover_apps()
    }

    fn launch_app(&self, app: &InstalledApp) -> Result<(), String> {
        self.inner.launch_app(app)
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        self.inner.open_url(url)
    }

    fn copy_image_to_clipboard(&self, image_path: &std::path::Path) -> Result<(), String> {
        super::clipboard::copy_image_file_to_clipboard(image_path)
    }

    fn search_browser_tabs(&self, query: &str) -> Result<Vec<BrowserTab>, String> {
        self.inner.search_browser_tabs(query)
    }

    fn focus_browser_tab(&self, target: &BrowserTabTarget) -> Result<(), String> {
        self.inner.focus_browser_tab(target)
    }

    fn search_processes(&self, query: &str) -> Result<Vec<ProcessMatch>, String> {
        self.inner.search_processes(query)
    }

    fn terminate_process(&self, pid: u32) -> Result<(), String> {
        self.inner.terminate_process(pid)
    }
}

pub(super) struct DesktopSearchIndex {
    inner: TantivySearchIndex,
}

impl DesktopSearchIndex {
    pub(super) fn open_or_create(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        Ok(Self {
            inner: TantivySearchIndex::open_or_create(path).map_err(|error| error.to_string())?,
        })
    }
}

impl SearchIndex for DesktopSearchIndex {
    fn is_configured(&self) -> bool {
        self.inner.is_configured()
    }

    fn search_item_ids(&self, query: &str, limit: usize) -> Result<Vec<String>, String> {
        self.inner
            .search_item_ids(query, limit)
            .map_err(|error| error.to_string())
    }

    fn replace_items(&self, items: &[SearchableItemDocument]) -> Result<SearchIndexStats, String> {
        self.inner
            .replace_items(items)
            .map_err(|error| error.to_string())
    }
}
