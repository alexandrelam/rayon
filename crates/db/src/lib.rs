use rayon_types::{SearchResultKind, SearchableItemDocument};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::directory::error::OpenDirectoryError;
use tantivy::directory::Directory;
use tantivy::directory::{MmapDirectory, RamDirectory};
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, TantivyDocument};

const INDEX_WRITER_HEAP_BYTES: usize = 50_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexStats {
    pub discovered_count: usize,
    pub indexed_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug)]
pub enum TantivySearchIndexError {
    Io(std::io::Error),
    Directory(OpenDirectoryError),
    Backend(tantivy::TantivyError),
}

impl fmt::Display for TantivySearchIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Directory(error) => write!(f, "{error}"),
            Self::Backend(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for TantivySearchIndexError {}

impl From<std::io::Error> for TantivySearchIndexError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<OpenDirectoryError> for TantivySearchIndexError {
    fn from(value: OpenDirectoryError) -> Self {
        Self::Directory(value)
    }
}

impl From<tantivy::TantivyError> for TantivySearchIndexError {
    fn from(value: tantivy::TantivyError) -> Self {
        Self::Backend(value)
    }
}

#[derive(Debug, Clone, Copy)]
struct SearchIndexFields {
    id: Field,
    kind: Field,
    title: Field,
    subtitle: Field,
    owner_plugin_id: Field,
    search_text: Field,
}

impl SearchIndexFields {
    fn build_schema() -> (Schema, Self) {
        let mut schema_builder = Schema::builder();
        let id = schema_builder.add_text_field("id", STRING | STORED);
        let kind = schema_builder.add_text_field("kind", STRING);
        let title = schema_builder.add_text_field("title", TEXT);
        let subtitle = schema_builder.add_text_field("subtitle", TEXT);
        let owner_plugin_id = schema_builder.add_text_field("owner_plugin_id", TEXT);
        let search_text = schema_builder.add_text_field("search_text", TEXT);
        let schema = schema_builder.build();

        (
            schema,
            Self {
                id,
                kind,
                title,
                subtitle,
                owner_plugin_id,
                search_text,
            },
        )
    }
}

pub struct TantivySearchIndex {
    index: Index,
    reader: IndexReader,
    fields: SearchIndexFields,
    path: Option<PathBuf>,
}

impl TantivySearchIndex {
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, TantivySearchIndexError> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(&path)?;

        let (schema, fields) = SearchIndexFields::build_schema();
        let (index, reader) = match Self::open_index(&path, schema.clone()) {
            Ok((index, reader)) => (index, reader),
            Err(TantivySearchIndexError::Backend(error)) if is_schema_mismatch(&error) => {
                rebuild_index_directory(&path)?;
                Self::open_index(&path, schema)?
            }
            Err(error) => return Err(error),
        };

        Ok(Self {
            index,
            reader,
            fields,
            path: Some(path),
        })
    }

    pub fn create_in_memory() -> Result<Self, TantivySearchIndexError> {
        let (schema, fields) = SearchIndexFields::build_schema();
        let (index, reader) = Self::open_index_in_directory(RamDirectory::create(), schema)?;

        Ok(Self {
            index,
            reader,
            fields,
            path: None,
        })
    }

    pub fn is_configured(&self) -> bool {
        true
    }

    pub fn search_item_ids(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, TantivySearchIndexError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let searcher = self.reader.searcher();
        let top_docs = if query.trim().is_empty() {
            searcher.search(&AllQuery, &TopDocs::with_limit(limit).order_by_score())?
        } else {
            let query_parser = QueryParser::for_index(
                &self.index,
                vec![
                    self.fields.title,
                    self.fields.subtitle,
                    self.fields.owner_plugin_id,
                    self.fields.search_text,
                ],
            );
            let (parsed_query, _errors) = query_parser.parse_query_lenient(query.trim());
            searcher.search(&parsed_query, &TopDocs::with_limit(limit).order_by_score())?
        };

        let mut item_ids = Vec::with_capacity(top_docs.len());
        for (_score, doc_address) in top_docs {
            let document = searcher.doc::<TantivyDocument>(doc_address)?;
            if let Some(item_id) = document
                .get_first(self.fields.id)
                .and_then(|value| value.as_str())
            {
                item_ids.push(item_id.to_string());
            }
        }

        Ok(item_ids)
    }

    pub fn replace_items(
        &self,
        items: &[SearchableItemDocument],
    ) -> Result<SearchIndexStats, TantivySearchIndexError> {
        let mut writer = self.index.writer(INDEX_WRITER_HEAP_BYTES)?;
        writer.delete_all_documents()?;

        let mut indexed_count = 0;
        let mut skipped_count = 0;
        for item in items {
            let searchable_fields = SearchableFields::from_item(item);
            if searchable_fields.is_empty() {
                skipped_count += 1;
                continue;
            }

            writer.add_document(doc!(
                self.fields.id => item.id.as_str(),
                self.fields.kind => search_kind(item.kind.clone()),
                self.fields.title => searchable_fields.title,
                self.fields.subtitle => searchable_fields.subtitle,
                self.fields.owner_plugin_id => searchable_fields.owner_plugin_id,
                self.fields.search_text => searchable_fields.search_text,
            ))?;
            indexed_count += 1;
        }

        writer.commit()?;
        self.reader.reload()?;

        Ok(SearchIndexStats {
            discovered_count: items.len(),
            indexed_count,
            skipped_count,
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn open_index(
        path: &Path,
        schema: Schema,
    ) -> Result<(Index, IndexReader), TantivySearchIndexError> {
        let directory = MmapDirectory::open(path)?;
        Self::open_index_in_directory(directory, schema)
    }

    fn open_index_in_directory<D: Directory + 'static>(
        directory: D,
        schema: Schema,
    ) -> Result<(Index, IndexReader), TantivySearchIndexError> {
        let index = Index::open_or_create(directory, schema)?;
        let reader = index.reader()?;
        Ok((index, reader))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchableFields {
    title: String,
    subtitle: String,
    owner_plugin_id: String,
    search_text: String,
}

impl SearchableFields {
    fn from_item(item: &SearchableItemDocument) -> Self {
        Self {
            title: prefix_search_terms(&item.title),
            subtitle: prefix_search_terms(item.subtitle.as_deref().unwrap_or_default()),
            owner_plugin_id: prefix_search_terms(
                item.owner_plugin_id.as_deref().unwrap_or_default(),
            ),
            search_text: prefix_search_terms(&item.search_text),
        }
    }

    fn is_empty(&self) -> bool {
        self.title.is_empty()
            && self.subtitle.is_empty()
            && self.owner_plugin_id.is_empty()
            && self.search_text.is_empty()
    }
}

fn search_kind(kind: SearchResultKind) -> &'static str {
    match kind {
        SearchResultKind::Command => "command",
        SearchResultKind::Application => "application",
        SearchResultKind::Bookmark => "bookmark",
        SearchResultKind::BrowserTab => "browser_tab",
    }
}

fn is_schema_mismatch(error: &tantivy::TantivyError) -> bool {
    match error {
        tantivy::TantivyError::SchemaError(message) => {
            message.contains("schema does not match")
                || message.contains("An index exists but the schema does not match")
        }
        _ => false,
    }
}

fn rebuild_index_directory(path: &Path) -> Result<(), TantivySearchIndexError> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn prefix_search_terms(text: &str) -> String {
    let mut prefixes = BTreeSet::new();

    for token in text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        let lowercase = token.to_lowercase();
        let char_count = lowercase.chars().count();
        if char_count < 2 {
            continue;
        }

        for prefix_len in 2..=char_count {
            prefixes.insert(lowercase.chars().take(prefix_len).collect::<String>());
        }
    }

    prefixes.into_iter().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_types::CommandId;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEST_INDEX_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_index_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let unique_id = NEXT_TEST_INDEX_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("rayon-tantivy-test-{nanos}-{unique_id}"))
    }

    fn test_index() -> TantivySearchIndex {
        TantivySearchIndex::open_or_create(unique_index_path()).unwrap()
    }

    fn searchable_item(
        id: &str,
        kind: SearchResultKind,
        title: &str,
        subtitle: Option<&str>,
        owner_plugin_id: Option<&str>,
        search_text: &str,
    ) -> SearchableItemDocument {
        SearchableItemDocument {
            id: CommandId::from(id),
            kind,
            title: title.into(),
            subtitle: subtitle.map(str::to_string),
            owner_plugin_id: owner_plugin_id.map(str::to_string),
            search_text: search_text.into(),
        }
    }

    #[test]
    fn search_is_empty_for_blank_query_without_docs() {
        let index = test_index();
        let results = index.search_item_ids("", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_matches_two_character_title_prefix() {
        let index = test_index();
        index
            .replace_items(&[searchable_item(
                "command:hello",
                SearchResultKind::Command,
                "Hello",
                None,
                Some("user.commands"),
                "hello greeting",
            )])
            .unwrap();

        let results = index.search_item_ids("he", 10).unwrap();
        assert_eq!(results, vec![String::from("command:hello")]);
    }

    #[test]
    fn search_matches_owner_prefix() {
        let index = test_index();
        index
            .replace_items(&[searchable_item(
                "command:hello",
                SearchResultKind::Command,
                "Hello",
                None,
                Some("user.commands"),
                "hello greeting",
            )])
            .unwrap();

        let results = index.search_item_ids("user", 10).unwrap();
        assert_eq!(results, vec![String::from("command:hello")]);
    }

    #[test]
    fn search_matches_application_metadata() {
        let index = test_index();
        index
            .replace_items(&[searchable_item(
                "app:macos:com.example.arc",
                SearchResultKind::Application,
                "Arc",
                Some("com.example.arc"),
                None,
                "Arc com.example.arc",
            )])
            .unwrap();

        let results = index.search_item_ids("arc", 10).unwrap();
        assert_eq!(results, vec![String::from("app:macos:com.example.arc")]);
    }

    #[test]
    fn blank_query_returns_ranked_documents() {
        let index = test_index();
        index
            .replace_items(&[searchable_item(
                "command:hello",
                SearchResultKind::Command,
                "Hello",
                None,
                Some("user.commands"),
                "hello greeting",
            )])
            .unwrap();

        let results = index.search_item_ids("", 10).unwrap();
        assert_eq!(results, vec![String::from("command:hello")]);
    }

    #[test]
    fn reindex_replaces_stale_documents() {
        let index = test_index();
        index
            .replace_items(&[searchable_item(
                "stale",
                SearchResultKind::Command,
                "Stale",
                None,
                None,
                "stale",
            )])
            .unwrap();
        index
            .replace_items(&[searchable_item(
                "fresh",
                SearchResultKind::Command,
                "Fresh",
                None,
                None,
                "fresh",
            )])
            .unwrap();

        let results = index.search_item_ids("stale", 10).unwrap();
        assert!(results.is_empty());
        let fresh_results = index.search_item_ids("fresh", 10).unwrap();
        assert_eq!(fresh_results, vec![String::from("fresh")]);
    }

    #[test]
    fn persists_index_on_disk() {
        let path = unique_index_path();
        let index = TantivySearchIndex::open_or_create(&path).unwrap();
        index
            .replace_items(&[searchable_item(
                "persisted",
                SearchResultKind::Command,
                "Persisted",
                None,
                None,
                "persisted",
            )])
            .unwrap();

        let reopened = TantivySearchIndex::open_or_create(&path).unwrap();
        let results = reopened.search_item_ids("persisted", 10).unwrap();
        assert_eq!(results, vec![String::from("persisted")]);
    }

    #[test]
    fn recreates_index_when_schema_changes() {
        let path = unique_index_path();
        fs::create_dir_all(&path).unwrap();

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("legacy_id", STRING | STORED);
        let legacy_schema = schema_builder.build();
        let directory = MmapDirectory::open(&path).unwrap();
        let legacy_index = Index::open_or_create(directory, legacy_schema).unwrap();
        let mut writer = legacy_index
            .writer::<TantivyDocument>(INDEX_WRITER_HEAP_BYTES)
            .unwrap();
        writer.commit().unwrap();

        let reopened = TantivySearchIndex::open_or_create(&path).unwrap();
        reopened
            .replace_items(&[searchable_item(
                "fresh",
                SearchResultKind::Command,
                "Fresh",
                None,
                None,
                "fresh",
            )])
            .unwrap();

        let results = reopened.search_item_ids("fresh", 10).unwrap();
        assert_eq!(results, vec![String::from("fresh")]);
    }

    #[test]
    fn searches_in_memory_indexes() {
        let index = TantivySearchIndex::create_in_memory().unwrap();
        index
            .replace_items(&[searchable_item(
                "browser-tab:chrome:window-1:2",
                SearchResultKind::BrowserTab,
                "Issue 15",
                Some("Google Chrome · https://github.com/alexandrelam/rayon/issues/15"),
                None,
                "Issue 15 https://github.com/alexandrelam/rayon/issues/15 chrome",
            )])
            .unwrap();

        let results = index.search_item_ids("rayon issue", 10).unwrap();
        assert_eq!(results, vec![String::from("browser-tab:chrome:window-1:2")]);
        assert!(index.path().is_none());
    }
}
