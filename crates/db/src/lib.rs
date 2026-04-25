use rayon_types::InstalledApp;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::directory::error::OpenDirectoryError;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, TantivyDocument};

const INDEX_WRITER_HEAP_BYTES: usize = 50_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIndexStats {
    pub discovered_count: usize,
    pub indexed_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug)]
pub enum TantivyAppIndexError {
    Io(std::io::Error),
    Directory(OpenDirectoryError),
    Backend(tantivy::TantivyError),
}

impl fmt::Display for TantivyAppIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Directory(error) => write!(f, "{error}"),
            Self::Backend(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for TantivyAppIndexError {}

impl From<std::io::Error> for TantivyAppIndexError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<OpenDirectoryError> for TantivyAppIndexError {
    fn from(value: OpenDirectoryError) -> Self {
        Self::Directory(value)
    }
}

impl From<tantivy::TantivyError> for TantivyAppIndexError {
    fn from(value: tantivy::TantivyError) -> Self {
        Self::Backend(value)
    }
}

#[derive(Debug, Clone, Copy)]
struct AppIndexFields {
    id: Field,
    title: Field,
    bundle_identifier: Field,
    bundle_name: Field,
}

impl AppIndexFields {
    fn build_schema() -> (Schema, Self) {
        let mut schema_builder = Schema::builder();
        let id = schema_builder.add_text_field("id", STRING | STORED);
        let title = schema_builder.add_text_field("title", TEXT);
        let bundle_identifier = schema_builder.add_text_field("bundle_identifier", TEXT);
        let bundle_name = schema_builder.add_text_field("bundle_name", TEXT);
        let schema = schema_builder.build();

        (
            schema,
            Self {
                id,
                title,
                bundle_identifier,
                bundle_name,
            },
        )
    }
}

pub struct TantivyAppIndex {
    index: Index,
    reader: IndexReader,
    fields: AppIndexFields,
    path: PathBuf,
}

impl TantivyAppIndex {
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, TantivyAppIndexError> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(&path)?;

        let (schema, fields) = AppIndexFields::build_schema();
        let directory = MmapDirectory::open(&path)?;
        let index = Index::open_or_create(directory, schema)?;
        let reader = index.reader()?;

        Ok(Self {
            index,
            reader,
            fields,
            path,
        })
    }

    pub fn is_configured(&self) -> bool {
        true
    }

    pub fn search_app_ids(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, TantivyAppIndexError> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.fields.title,
                self.fields.bundle_identifier,
                self.fields.bundle_name,
            ],
        );
        let (parsed_query, _errors) = query_parser.parse_query_lenient(query);
        let top_docs =
            searcher.search(&parsed_query, &TopDocs::with_limit(limit).order_by_score())?;

        let mut app_ids = Vec::with_capacity(top_docs.len());
        for (_score, doc_address) in top_docs {
            let document = searcher.doc::<TantivyDocument>(doc_address)?;
            if let Some(app_id) = document
                .get_first(self.fields.id)
                .and_then(|value| value.as_str())
            {
                app_ids.push(app_id.to_string());
            }
        }

        Ok(app_ids)
    }

    pub fn reindex_apps(
        &self,
        apps: &[InstalledApp],
    ) -> Result<AppIndexStats, TantivyAppIndexError> {
        let mut writer = self.index.writer(INDEX_WRITER_HEAP_BYTES)?;
        writer.delete_all_documents()?;

        let mut indexed_count = 0;
        let mut skipped_count = 0;
        for app in apps {
            let fields = AppDocumentFields::from_app(app);
            if fields.is_empty() {
                skipped_count += 1;
                continue;
            }

            writer.add_document(doc!(
                self.fields.id => app.id.as_str(),
                self.fields.title => fields.title,
                self.fields.bundle_identifier => fields.bundle_identifier,
                self.fields.bundle_name => fields.bundle_name,
            ))?;
            indexed_count += 1;
        }

        writer.commit()?;
        self.reader.reload()?;

        Ok(AppIndexStats {
            discovered_count: apps.len(),
            indexed_count,
            skipped_count,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppDocumentFields {
    title: String,
    bundle_identifier: String,
    bundle_name: String,
}

impl AppDocumentFields {
    fn from_app(app: &InstalledApp) -> Self {
        Self {
            title: app.title.trim().to_string(),
            bundle_identifier: app
                .bundle_identifier
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_string(),
            bundle_name: app_bundle_name(app),
        }
    }

    fn is_empty(&self) -> bool {
        self.title.is_empty() && self.bundle_identifier.is_empty() && self.bundle_name.is_empty()
    }
}

fn app_bundle_name(app: &InstalledApp) -> String {
    Path::new(&app.path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon_types::CommandId;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_index_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rayon-tantivy-test-{nanos}"))
    }

    fn test_index() -> TantivyAppIndex {
        TantivyAppIndex::open_or_create(unique_index_path()).unwrap()
    }

    fn installed_app(
        id: &str,
        title: &str,
        bundle_identifier: Option<&str>,
        path: &str,
    ) -> InstalledApp {
        InstalledApp {
            id: CommandId::from(id),
            title: title.into(),
            bundle_identifier: bundle_identifier.map(str::to_string),
            path: path.into(),
        }
    }

    #[test]
    fn search_is_empty_for_blank_query() {
        let index = test_index();

        let results = index.search_app_ids("   ", 10).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn reindex_and_search_by_title() {
        let index = test_index();
        let apps = vec![
            installed_app(
                "app:macos:com.example.arc",
                "Arc",
                Some("com.example.arc"),
                "/Applications/Arc.app",
            ),
            installed_app(
                "app:macos:com.example.notes",
                "Notes",
                Some("com.apple.Notes"),
                "/System/Applications/Notes.app",
            ),
        ];

        let stats = index.reindex_apps(&apps).unwrap();
        let results = index.search_app_ids("arc", 10).unwrap();

        assert_eq!(
            stats,
            AppIndexStats {
                discovered_count: 2,
                indexed_count: 2,
                skipped_count: 0,
            }
        );
        assert_eq!(results, vec!["app:macos:com.example.arc"]);
    }

    #[test]
    fn search_matches_bundle_identifier() {
        let index = test_index();
        let apps = vec![installed_app(
            "app:macos:com.example.arc",
            "Browser",
            Some("com.example.arc"),
            "/Applications/Arc.app",
        )];

        index.reindex_apps(&apps).unwrap();

        let results = index.search_app_ids("com.example.arc", 10).unwrap();

        assert_eq!(results, vec!["app:macos:com.example.arc"]);
    }

    #[test]
    fn reindex_replaces_stale_documents() {
        let index = test_index();
        let old_apps = vec![installed_app(
            "app:macos:com.example.arc",
            "Arc",
            Some("com.example.arc"),
            "/Applications/Arc.app",
        )];
        let new_apps = vec![installed_app(
            "app:macos:com.example.notes",
            "Notes",
            Some("com.apple.Notes"),
            "/System/Applications/Notes.app",
        )];

        index.reindex_apps(&old_apps).unwrap();
        index.reindex_apps(&new_apps).unwrap();

        assert!(index.search_app_ids("arc", 10).unwrap().is_empty());
        assert_eq!(
            index.search_app_ids("notes", 10).unwrap(),
            vec!["app:macos:com.example.notes"]
        );
    }

    #[test]
    fn persists_index_on_disk() {
        let path = unique_index_path();
        let index = TantivyAppIndex::open_or_create(&path).unwrap();
        let apps = vec![installed_app(
            "app:macos:com.example.arc",
            "Arc",
            Some("com.example.arc"),
            "/Applications/Arc.app",
        )];

        index.reindex_apps(&apps).unwrap();
        assert_eq!(index.path(), path.as_path());
        drop(index);

        let reopened = TantivyAppIndex::open_or_create(&path).unwrap();

        assert_eq!(
            reopened.search_app_ids("arc", 10).unwrap(),
            vec!["app:macos:com.example.arc"]
        );
    }
}
