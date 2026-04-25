use plist::Value;
use rayon_types::{CommandId, InstalledApp};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const APPLICATIONS_DIR: &str = "/Applications";
const SYSTEM_APPLICATIONS_DIR: &str = "/System/Applications";

#[derive(Debug, Default, Clone, Copy)]
pub struct MacOsAppManager;

impl MacOsAppManager {
    pub fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
        let mut candidates = self.spotlight_candidates()?;
        if candidates.is_empty() {
            candidates = self.fallback_candidates();
        }

        let mut apps_by_id: HashMap<String, InstalledApp> = HashMap::new();
        for path in candidates {
            if let Some(app) = parse_app_bundle(&path) {
                let key = app.id.to_string();
                match apps_by_id.get(&key) {
                    Some(existing) if compare_app_priority(existing, &app) != Ordering::Greater => {
                    }
                    _ => {
                        apps_by_id.insert(key, app);
                    }
                }
            }
        }

        let mut apps: Vec<_> = apps_by_id.into_values().collect();
        apps.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
        Ok(apps)
    }

    pub fn launch_app(&self, app: &InstalledApp) -> Result<(), String> {
        let status = Command::new("/usr/bin/open")
            .arg(&app.path)
            .status()
            .map_err(|error| format!("failed to launch {}: {error}", app.title))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("failed to launch {}", app.title))
        }
    }

    fn spotlight_candidates(&self) -> Result<Vec<PathBuf>, String> {
        let output = Command::new("mdfind")
            .arg(r#"kMDItemContentType == "com.apple.application-bundle""#)
            .output()
            .map_err(|error| format!("failed to run mdfind: {error}"))?;

        if !output.status.success() {
            return Err(String::from("mdfind returned a non-zero exit status"));
        }

        Ok(parse_spotlight_output(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    fn fallback_candidates(&self) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        for root in preferred_roots() {
            collect_app_bundles(&root, 2, &mut candidates);
        }

        candidates
    }
}

fn parse_spotlight_output(output: &str) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    output
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(".app"))
        .filter_map(|line| {
            let path = PathBuf::from(line);
            if seen.insert(path.clone()) {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

fn collect_app_bundles(root: &Path, depth: usize, candidates: &mut Vec<PathBuf>) {
    if depth == 0 || !root.exists() {
        return;
    }

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if is_app_bundle(&path) {
            candidates.push(path);
            continue;
        }

        if path.is_dir() {
            collect_app_bundles(&path, depth - 1, candidates);
        }
    }
}

fn preferred_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from(APPLICATIONS_DIR),
        PathBuf::from(SYSTEM_APPLICATIONS_DIR),
    ];

    if let Some(home) = env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }

    roots
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
}

fn compare_app_priority(left: &InstalledApp, right: &InstalledApp) -> Ordering {
    app_path_rank(&left.path).cmp(&app_path_rank(&right.path))
}

fn app_path_rank(path: &str) -> usize {
    let user_applications = env::var("HOME")
        .map(|home| format!("{home}/Applications"))
        .ok();

    if path.starts_with(APPLICATIONS_DIR) {
        0
    } else if user_applications
        .as_ref()
        .is_some_and(|prefix| path.starts_with(prefix))
    {
        1
    } else if path.starts_with(SYSTEM_APPLICATIONS_DIR) {
        2
    } else {
        3
    }
}

fn parse_app_bundle(path: &Path) -> Option<InstalledApp> {
    let plist_path = path.join("Contents").join("Info.plist");
    let plist = Value::from_file(&plist_path).ok()?;
    let dict = plist.into_dictionary()?;

    let bundle_identifier = dict
        .get("CFBundleIdentifier")
        .and_then(Value::as_string)
        .map(ToOwned::to_owned);
    let title = dict
        .get("CFBundleDisplayName")
        .and_then(Value::as_string)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            dict.get("CFBundleName")
                .and_then(Value::as_string)
                .filter(|value| !value.trim().is_empty())
        })
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(OsStr::to_str)
                .unwrap_or("Application")
                .to_string()
        });

    let absolute_path = fs::canonicalize(path).ok()?.to_string_lossy().to_string();
    let id = bundle_identifier
        .as_deref()
        .map(|bundle_id| format!("app:macos:{bundle_id}"))
        .unwrap_or_else(|| format!("app:macos:path:{absolute_path}"));

    Some(InstalledApp {
        id: CommandId::from(id),
        title,
        bundle_identifier,
        path: absolute_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_non_app_lines_from_spotlight() {
        let output = "/Applications/Arc.app\n/tmp/not-an-app\n/System/Applications/Preview.app\n";
        let candidates = parse_spotlight_output(output);

        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|path| is_app_bundle(path)));
    }

    #[test]
    fn applications_directory_has_highest_priority() {
        let better = InstalledApp {
            id: CommandId::from("app:macos:com.example.arc"),
            title: "Arc".into(),
            bundle_identifier: Some("com.example.arc".into()),
            path: "/Applications/Arc.app".into(),
        };
        let worse = InstalledApp {
            id: CommandId::from("app:macos:com.example.arc"),
            title: "Arc".into(),
            bundle_identifier: Some("com.example.arc".into()),
            path: "/System/Applications/Arc.app".into(),
        };

        assert_eq!(compare_app_priority(&better, &worse), Ordering::Less);
    }
}
