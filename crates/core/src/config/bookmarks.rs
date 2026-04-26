use super::manifest::ManifestBookmark;
use rayon_types::{BookmarkDefinition, CommandId};
use url::Url;

pub(super) fn load_bookmarks(
    plugin_id: &str,
    bookmarks: Vec<ManifestBookmark>,
) -> Result<Vec<BookmarkDefinition>, String> {
    let mut bookmark_definitions = Vec::with_capacity(bookmarks.len());

    for bookmark in bookmarks {
        validate_bookmark_url(&bookmark.url)?;
        bookmark_definitions.push(BookmarkDefinition {
            id: CommandId::from(bookmark.id),
            title: bookmark.title,
            subtitle: bookmark.subtitle,
            owner_plugin_id: plugin_id.to_string(),
            url: bookmark.url,
            keywords: bookmark.keywords.unwrap_or_default(),
        });
    }

    Ok(bookmark_definitions)
}

fn validate_bookmark_url(raw_url: &str) -> Result<(), String> {
    let parsed = Url::parse(raw_url)
        .map_err(|error| format!("invalid bookmark url '{raw_url}': {error}"))?;
    if parsed.scheme().is_empty() {
        return Err(format!(
            "invalid bookmark url '{raw_url}': missing URL scheme"
        ));
    }

    Ok(())
}
