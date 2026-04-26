use super::manifest::PluginManifest;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn config_dir() -> Result<PathBuf, String> {
    if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg_config_home).join("rayon"));
    }

    let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".config").join("rayon"))
}

pub(super) fn manifest_paths(config_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(config_dir).map_err(|error| {
        format!(
            "failed to read config directory {}: {error}",
            config_dir.display()
        )
    })?;

    let mut manifest_paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    manifest_paths.sort();
    Ok(manifest_paths)
}

pub(super) fn load_manifest(path: &Path) -> Result<PluginManifest, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read plugin manifest {}: {error}", path.display()))?;
    toml::from_str(&source).map_err(|error| {
        format!(
            "failed to parse plugin manifest {}: {error}",
            path.display()
        )
    })
}
