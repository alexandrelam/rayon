use plist::Value;
use rayon_types::{BrowserTab, BrowserTabTarget, CommandId, InstalledApp, ProcessMatch};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

const APPLICATIONS_DIR: &str = "/Applications";
const SYSTEM_APPLICATIONS_DIR: &str = "/System/Applications";
const APPLE_SCRIPT_FIELD_SEPARATOR: char = '\u{001f}';
const APPLE_SCRIPT_RECORD_SEPARATOR: char = '\u{001e}';

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

    pub fn open_url(&self, url: &str) -> Result<(), String> {
        let status = Command::new("/usr/bin/open")
            .arg(url)
            .status()
            .map_err(|error| format!("failed to open {url}: {error}"))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("failed to open {url}"))
        }
    }

    pub fn search_browser_tabs(&self, query: &str) -> Result<Vec<BrowserTab>, String> {
        let trimmed_query = query.trim();
        let output = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(chrome_tabs_list_script())
            .output()
            .map_err(|error| format!("failed to list Chrome tabs: {error}"))?;

        if !output.status.success() {
            return Err(stderr_or_stdout(&output)
                .unwrap_or_else(|| "failed to list Chrome tabs".to_string()));
        }

        let tabs = parse_chrome_tabs_output(&String::from_utf8_lossy(&output.stdout))?;
        Ok(filter_browser_tabs(&tabs, trimmed_query))
    }

    pub fn focus_browser_tab(&self, target: &BrowserTabTarget) -> Result<(), String> {
        if target.browser != "chrome" {
            return Err(format!("unsupported browser target: {}", target.browser));
        }

        let mut last_error = None;
        for attempt in 0..4 {
            let output = Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg(chrome_focus_tab_script(&target.window_id, target.tab_index))
                .output()
                .map_err(|error| format!("failed to focus Chrome tab: {error}"))?;

            if output.status.success() {
                return Ok(());
            }

            last_error = stderr_or_stdout(&output);
            if attempt < 3 {
                thread::sleep(Duration::from_millis(100));
            }
        }

        Err(last_error.unwrap_or_else(|| "failed to focus Chrome tab".to_string()))
    }

    pub fn search_processes(&self, query: &str) -> Result<Vec<ProcessMatch>, String> {
        let trimmed_query = query.trim();
        if trimmed_query.is_empty() {
            return Ok(Vec::new());
        }

        if let Some(port) = parse_port_query(trimmed_query) {
            return self.search_processes_by_port(port);
        }

        let processes = self.list_processes()?;
        Ok(filter_processes_by_name(&processes, trimmed_query))
    }

    pub fn terminate_process(&self, pid: u32) -> Result<(), String> {
        let status = Command::new("/bin/kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .map_err(|error| format!("failed to terminate pid {pid}: {error}"))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("failed to terminate pid {pid}"))
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

    fn list_processes(&self) -> Result<Vec<ProcessMatch>, String> {
        let output = Command::new("ps")
            .args(["-axo", "pid=,comm=,args="])
            .output()
            .map_err(|error| format!("failed to run ps: {error}"))?;

        if !output.status.success() {
            return Err(String::from("ps returned a non-zero exit status"));
        }

        Ok(parse_ps_output(&String::from_utf8_lossy(&output.stdout)))
    }

    fn search_processes_by_port(&self, port: u16) -> Result<Vec<ProcessMatch>, String> {
        let output = Command::new("lsof")
            .args(["-nP", "-i", &format!(":{port}"), "-F", "pcn"])
            .output()
            .map_err(|error| format!("failed to run lsof: {error}"))?;

        if !output.status.success() && !output.stdout.is_empty() {
            return Err(String::from("lsof returned a non-zero exit status"));
        }

        Ok(parse_lsof_output(
            &String::from_utf8_lossy(&output.stdout),
            port,
        ))
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

fn stderr_or_stdout(output: &std::process::Output) -> Option<String> {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return Some(stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }

    None
}

fn chrome_tabs_list_script() -> &'static str {
    r#"
set fieldSep to character id 31
set recordSep to character id 30

tell application "System Events"
    if not (exists process "Google Chrome") then
        return ""
    end if
end tell

tell application "Google Chrome"
    set rows to {}
    repeat with w in every window
        set windowId to (id of w as text)
        set windowIndex to (index of w as text)
        set activeIndex to (active tab index of w as text)
        set tabIndex to 0
        repeat with t in every tab of w
            set tabIndex to tabIndex + 1
            set rowParts to {windowId, windowIndex, activeIndex, (tabIndex as text), (title of t as text), (URL of t as text)}
            set oldDelims to AppleScript's text item delimiters
            set AppleScript's text item delimiters to fieldSep
            set end of rows to (rowParts as text)
            set AppleScript's text item delimiters to oldDelims
        end repeat
    end repeat
end tell

set oldDelims to AppleScript's text item delimiters
set AppleScript's text item delimiters to recordSep
set outputText to rows as text
set AppleScript's text item delimiters to oldDelims
return outputText
"#
}

fn chrome_focus_tab_script(window_id: &str, tab_index: u32) -> String {
    format!(
        r#"
tell application "System Events"
    if not (exists process "Google Chrome") then
        error "Google Chrome is not running"
    end if
end tell

tell application "Google Chrome"
    if not (exists (first window whose id is "{}")) then
        error "Chrome window not found"
    end if
    set targetWindow to first window whose id is "{}"
    if (count of tabs of targetWindow) < {} then
        error "Chrome tab not found"
    end if
    set index of targetWindow to 1
    set active tab index of targetWindow to {}
    activate
end tell
"#,
        apple_script_string(window_id),
        apple_script_string(window_id),
        tab_index,
        tab_index
    )
}

fn apple_script_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_chrome_tabs_output(output: &str) -> Result<Vec<BrowserTab>, String> {
    let trimmed_output = output.trim();
    if trimmed_output.is_empty() {
        return Ok(Vec::new());
    }

    trimmed_output
        .split(APPLE_SCRIPT_RECORD_SEPARATOR)
        .filter(|line| !line.trim().is_empty())
        .map(parse_chrome_tab_row)
        .collect()
}

fn parse_chrome_tab_row(row: &str) -> Result<BrowserTab, String> {
    let parts = row
        .split(APPLE_SCRIPT_FIELD_SEPARATOR)
        .map(str::trim)
        .collect::<Vec<_>>();

    if parts.len() != 6 {
        return Err(format!("invalid Chrome tab row: {row}"));
    }

    Ok(BrowserTab {
        browser: "chrome".into(),
        window_id: parts[0].to_string(),
        window_index: parts[1]
            .parse()
            .map_err(|error| format!("invalid Chrome window index '{}': {error}", parts[1]))?,
        active_tab_index: parts[2]
            .parse()
            .map_err(|error| format!("invalid Chrome active tab index '{}': {error}", parts[2]))?,
        tab_index: parts[3]
            .parse()
            .map_err(|error| format!("invalid Chrome tab index '{}': {error}", parts[3]))?,
        title: parts[4].to_string(),
        url: parts[5].to_string(),
    })
}

fn filter_browser_tabs(tabs: &[BrowserTab], query: &str) -> Vec<BrowserTab> {
    let normalized_query = query.trim().to_lowercase();
    if normalized_query.is_empty() {
        let mut all_tabs = tabs.to_vec();
        all_tabs.sort_by(|left, right| {
            right
                .is_active()
                .cmp(&left.is_active())
                .then_with(|| left.window_index.cmp(&right.window_index))
                .then_with(|| left.tab_index.cmp(&right.tab_index))
        });
        return all_tabs;
    }

    let mut matches = tabs
        .iter()
        .filter_map(|tab| {
            let score = browser_tab_match_score(tab, &normalized_query)?;
            Some((score, tab.clone()))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.window_index.cmp(&right.1.window_index))
            .then_with(|| left.1.tab_index.cmp(&right.1.tab_index))
    });

    matches.into_iter().map(|(_, tab)| tab).collect()
}

fn browser_tab_match_score(tab: &BrowserTab, query: &str) -> Option<(u8, u8, usize, usize)> {
    let title = tab.title.to_lowercase();
    let url = tab.url.to_lowercase();

    if tab.is_active() && (title.contains(query) || url.contains(query)) {
        return Some((0, 0, tab.window_index as usize, tab.tab_index as usize));
    }
    if title.starts_with(query) {
        return Some((0, 1, tab.window_index as usize, tab.tab_index as usize));
    }
    if title.contains(query) {
        return Some((0, 2, tab.window_index as usize, tab.tab_index as usize));
    }
    if url.starts_with(query) {
        return Some((1, 0, tab.window_index as usize, tab.tab_index as usize));
    }
    if url.contains(query) {
        return Some((1, 1, tab.window_index as usize, tab.tab_index as usize));
    }

    None
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

fn parse_port_query(query: &str) -> Option<u16> {
    let normalized = query.trim().to_lowercase();
    let candidate = normalized
        .strip_prefix("port ")
        .unwrap_or(normalized.as_str())
        .trim();

    if candidate.is_empty()
        || !candidate
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return None;
    }

    candidate.parse().ok()
}

fn parse_ps_output(output: &str) -> Vec<ProcessMatch> {
    let mut processes = Vec::new();

    for line in output.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            continue;
        }

        let mut columns = trimmed_line.split_whitespace();
        let Some(pid_column) = columns.next() else {
            continue;
        };
        let Some(command_path) = columns.next() else {
            continue;
        };
        let pid = match pid_column.parse::<u32>() {
            Ok(pid) => pid,
            Err(_) => continue,
        };

        let command = if let Some(index) = trimmed_line.find(command_path) {
            trimmed_line[index..].to_string()
        } else {
            command_path.to_string()
        };

        processes.push(ProcessMatch {
            pid,
            display_name: display_name_from_command(command_path),
            executable_name: executable_name_from_command(command_path),
            command,
            matched_ports: Vec::new(),
        });
    }

    processes
}

fn filter_processes_by_name(processes: &[ProcessMatch], query: &str) -> Vec<ProcessMatch> {
    let normalized_query = query.to_lowercase();
    let mut matches = Vec::new();

    for process in processes {
        let haystacks = [
            process.display_name.to_lowercase(),
            process.executable_name.to_lowercase(),
            process.command.to_lowercase(),
        ];
        if haystacks
            .iter()
            .any(|haystack| haystack.contains(&normalized_query))
        {
            matches.push(process.clone());
        }
    }

    matches.sort_by(|left, right| compare_process_matches(left, right, &normalized_query));
    matches
}

fn compare_process_matches(left: &ProcessMatch, right: &ProcessMatch, query: &str) -> Ordering {
    let left_prefix = left.display_name.to_lowercase().starts_with(query);
    let right_prefix = right.display_name.to_lowercase().starts_with(query);

    left_prefix
        .cmp(&right_prefix)
        .reverse()
        .then_with(|| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
        })
        .then_with(|| left.pid.cmp(&right.pid))
}

fn parse_lsof_output(output: &str, port: u16) -> Vec<ProcessMatch> {
    let mut processes = Vec::new();
    let mut current_pid: Option<u32> = None;
    let mut current_command: Option<String> = None;
    let mut current_ports: Vec<u16> = Vec::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        let (prefix, value) = line.split_at(1);
        match prefix {
            "p" => {
                if let Some(process) = build_lsof_process(
                    current_pid,
                    current_command.take(),
                    std::mem::take(&mut current_ports),
                ) {
                    processes.push(process);
                }
                current_pid = value.parse().ok();
            }
            "c" => {
                current_command = Some(value.to_string());
            }
            "n" => {
                if let Some(parsed_port) = parse_port_from_lsof_name(value) {
                    current_ports.push(parsed_port);
                } else {
                    current_ports.push(port);
                }
            }
            _ => {}
        }
    }

    if let Some(process) = build_lsof_process(current_pid, current_command, current_ports) {
        processes.push(process);
    }

    dedupe_port_matches(processes)
}

fn build_lsof_process(
    pid: Option<u32>,
    command: Option<String>,
    mut ports: Vec<u16>,
) -> Option<ProcessMatch> {
    let pid = pid?;
    let command = command?;
    ports.sort_unstable();
    ports.dedup();
    Some(ProcessMatch {
        pid,
        display_name: display_name_from_command(&command),
        executable_name: executable_name_from_command(&command),
        command: command.clone(),
        matched_ports: ports,
    })
}

fn dedupe_port_matches(processes: Vec<ProcessMatch>) -> Vec<ProcessMatch> {
    let mut by_pid: HashMap<u32, ProcessMatch> = HashMap::new();

    for process in processes {
        by_pid
            .entry(process.pid)
            .and_modify(|existing| {
                existing
                    .matched_ports
                    .extend(process.matched_ports.iter().copied());
                existing.matched_ports.sort_unstable();
                existing.matched_ports.dedup();
            })
            .or_insert(process);
    }

    let mut deduped: Vec<_> = by_pid.into_values().collect();
    deduped.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then(left.pid.cmp(&right.pid))
    });
    deduped
}

fn parse_port_from_lsof_name(value: &str) -> Option<u16> {
    let port_segment = value
        .rsplit(':')
        .next()?
        .split("->")
        .next()?
        .trim_matches(|character: char| !character.is_ascii_digit());
    port_segment.parse().ok()
}

fn display_name_from_command(command: &str) -> String {
    let executable = executable_name_from_command(command);
    executable
        .strip_suffix(".app")
        .unwrap_or(executable.as_str())
        .to_string()
}

fn executable_name_from_command(command: &str) -> String {
    Path::new(command)
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(command)
        .to_string()
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

    #[test]
    fn parses_port_queries_from_prefix_or_digits() {
        assert_eq!(parse_port_query("8080"), Some(8080));
        assert_eq!(parse_port_query("port 3000"), Some(3000));
        assert_eq!(parse_port_query("preview"), None);
    }

    #[test]
    fn parses_process_rows_from_ps_output() {
        let output = "  123 /Applications/Arc.app/Contents/MacOS/Arc /Applications/Arc.app/Contents/MacOS/Arc --flag\n  777 /usr/bin/node node server.js\n";
        let processes = parse_ps_output(output);

        assert_eq!(processes.len(), 2);
        assert_eq!(processes[0].display_name, "Arc");
        assert_eq!(processes[1].display_name, "node");
        assert_eq!(processes[1].command, "/usr/bin/node node server.js");
    }

    #[test]
    fn filters_processes_by_name_and_prefix_rank() {
        let processes = vec![
            ProcessMatch {
                pid: 10,
                display_name: "Preview".into(),
                executable_name: "Preview".into(),
                command: "/Applications/Preview.app".into(),
                matched_ports: Vec::new(),
            },
            ProcessMatch {
                pid: 20,
                display_name: "Arc".into(),
                executable_name: "Arc".into(),
                command: "/Applications/Arc.app".into(),
                matched_ports: Vec::new(),
            },
        ];

        let matches = filter_processes_by_name(&processes, "ar");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].display_name, "Arc");
    }

    #[test]
    fn parses_lsof_output_into_pid_rows() {
        let output =
            "p123\ncnode\nnTCP *:8080 (LISTEN)\np124\ncpython\nnTCP 127.0.0.1:8080 (LISTEN)\n";
        let matches = parse_lsof_output(output, 8080);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].matched_ports, vec![8080]);
        assert_eq!(matches[1].matched_ports, vec![8080]);
    }

    #[test]
    fn empty_browser_tab_query_returns_all_tabs_with_active_first() {
        let tabs = vec![
            BrowserTab {
                browser: "chrome".into(),
                window_id: "window-1".into(),
                window_index: 1,
                active_tab_index: 2,
                tab_index: 1,
                title: "First".into(),
                url: "https://example.com/1".into(),
            },
            BrowserTab {
                browser: "chrome".into(),
                window_id: "window-1".into(),
                window_index: 1,
                active_tab_index: 2,
                tab_index: 2,
                title: "Second".into(),
                url: "https://example.com/2".into(),
            },
        ];

        let matches = filter_browser_tabs(&tabs, "");

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].title, "Second");
        assert_eq!(matches[1].title, "First");
    }

    #[test]
    fn chrome_focus_tab_script_uses_single_activation_path() {
        let script = chrome_focus_tab_script("window-1", 2);

        assert!(script.contains("set active tab index of targetWindow to 2"));
        assert!(script.contains("\n    activate\n"));
        assert!(!script.contains("/usr/bin/open"));
    }
}
