mod bookmarks;
mod discovery;
mod loader;
pub(crate) mod manifest;

#[cfg(test)]
mod tests;

use crate::CommandProvider;
use std::collections::HashSet;
use std::sync::Arc;

pub use loader::LoadedConfig;

pub fn load_config() -> Result<LoadedConfig, String> {
    let config_dir = discovery::config_dir()?;
    if !config_dir.exists() {
        return Ok(LoadedConfig {
            command_providers: Vec::new(),
            bookmarks: Vec::new(),
        });
    }

    let mut command_providers: Vec<Arc<dyn CommandProvider>> = Vec::new();
    let mut bookmarks = Vec::new();
    let mut bookmark_ids = HashSet::new();

    for manifest_path in discovery::manifest_paths(&config_dir)? {
        let manifest = discovery::load_manifest(&manifest_path)?;
        let loaded =
            loader::load_manifest_bundle(manifest_path.parent().unwrap_or(&config_dir), manifest)?;

        if !loaded.command_provider.is_empty() {
            command_providers.push(Arc::new(loaded.command_provider));
        }
        for bookmark in loaded.bookmarks {
            if !bookmark_ids.insert(bookmark.id.to_string()) {
                return Err(format!("duplicate bookmark id registered: {}", bookmark.id));
            }
            bookmarks.push(bookmark);
        }
    }

    Ok(LoadedConfig {
        command_providers,
        bookmarks,
    })
}
