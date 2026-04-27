use rayon_core::{CommandError, CommandProvider, InteractiveSessionSubmitOutcome};
use rayon_types::{
    CommandDefinition, CommandExecutionRequest, CommandExecutionResult, CommandId,
    CommandInputMode, InteractiveSessionCompletionBehavior, InteractiveSessionMetadata,
    InteractiveSessionResult,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const CLIPBOARD_COMMAND_ID: &str = "clipboard";
const MAX_CLIPBOARD_HISTORY_ENTRIES: usize = 10;

pub trait ClipboardAccess: Send + Sync {
    fn read_text(&self) -> Result<Option<String>, String>;
    fn write_text(&self, text: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardHistoryItem {
    pub id: u64,
    pub text: String,
}

pub struct ClipboardHistoryService {
    access: Arc<dyn ClipboardAccess>,
    store: ClipboardHistoryStore,
}

impl ClipboardHistoryService {
    pub fn new(access: Arc<dyn ClipboardAccess>, path: PathBuf) -> Result<Self, String> {
        Ok(Self {
            access,
            store: ClipboardHistoryStore::new(path, MAX_CLIPBOARD_HISTORY_ENTRIES)?,
        })
    }

    pub fn sync_current_clipboard(&self) -> Result<(), String> {
        if let Some(text) = self.access.read_text()? {
            self.store.record_text(&text)?;
        }

        Ok(())
    }

    pub fn record_text(&self, text: &str) -> Result<(), String> {
        self.store.record_text(text)
    }

    pub fn recent_entries(&self) -> Vec<ClipboardHistoryItem> {
        self.store.entries()
    }

    pub fn copy_entry(&self, entry_id: u64) -> Result<ClipboardHistoryItem, String> {
        let entry = self
            .store
            .entry(entry_id)
            .ok_or_else(|| format!("unknown clipboard item: {entry_id}"))?;
        self.access.write_text(&entry.text)?;
        self.store.record_text(&entry.text)?;
        Ok(entry)
    }
}

pub struct ClipboardHistoryProvider {
    clipboard: Arc<ClipboardHistoryService>,
}

impl ClipboardHistoryProvider {
    pub fn new(clipboard: Arc<ClipboardHistoryService>) -> Self {
        Self { clipboard }
    }
}

impl CommandProvider for ClipboardHistoryProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![CommandDefinition {
            id: CommandId::from(CLIPBOARD_COMMAND_ID),
            title: "Clipboard History".into(),
            subtitle: Some("Browse and recopy your recent clipboard items".into()),
            owner_plugin_id: "builtin.clipboard".into(),
            keywords: vec![
                "clipboard".into(),
                "copy".into(),
                "paste".into(),
                "history".into(),
            ],
            close_launcher_on_success: false,
            input_mode: CommandInputMode::Structured,
            arguments: Vec::new(),
        }]
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        Err(CommandError::UnknownCommand(request.command_id.clone()))
    }

    fn start_interactive_session(
        &self,
        command_id: &CommandId,
    ) -> Result<Option<InteractiveSessionMetadata>, CommandError> {
        if command_id.as_str() != CLIPBOARD_COMMAND_ID {
            return Ok(None);
        }

        Ok(Some(InteractiveSessionMetadata {
            session_id: String::new(),
            command_id: command_id.clone(),
            title: "Clipboard History".into(),
            subtitle: Some("Search your 10 most recent clipboard entries".into()),
            input_placeholder: "Search clipboard history".into(),
            completion_behavior: InteractiveSessionCompletionBehavior::HideLauncherAndRestoreFocus,
        }))
    }

    fn search_interactive_session(
        &self,
        session: &InteractiveSessionMetadata,
        query: &str,
    ) -> Result<Vec<InteractiveSessionResult>, CommandError> {
        if session.command_id.as_str() != CLIPBOARD_COMMAND_ID {
            return Err(CommandError::UnknownCommand(session.command_id.clone()));
        }

        Ok(filter_entries(&self.clipboard.recent_entries(), query)
            .into_iter()
            .map(to_session_result)
            .collect())
    }

    fn submit_interactive_session(
        &self,
        session: &InteractiveSessionMetadata,
        _query: &str,
        item_id: &str,
    ) -> Result<InteractiveSessionSubmitOutcome, CommandError> {
        if session.command_id.as_str() != CLIPBOARD_COMMAND_ID {
            return Err(CommandError::UnknownCommand(session.command_id.clone()));
        }

        let entry_id = item_id.parse::<u64>().map_err(|_| {
            CommandError::InvalidArguments(format!("invalid clipboard item id: {item_id}"))
        })?;
        self.clipboard
            .copy_entry(entry_id)
            .map_err(CommandError::ExecutionFailed)?;

        Ok(InteractiveSessionSubmitOutcome::Completed(
            CommandExecutionResult {
                output: "copied clipboard item".into(),
            },
        ))
    }
}

#[derive(Clone)]
struct ClipboardHistoryStore {
    path: PathBuf,
    max_entries: usize,
    state: Arc<Mutex<ClipboardHistoryState>>,
}

impl ClipboardHistoryStore {
    fn new(path: PathBuf, max_entries: usize) -> Result<Self, String> {
        let state = load_state(&path, max_entries)?;
        Ok(Self {
            path,
            max_entries,
            state: Arc::new(Mutex::new(state)),
        })
    }

    fn entries(&self) -> Vec<ClipboardHistoryItem> {
        match self.state.lock() {
            Ok(state) => state.entries.clone(),
            Err(poisoned) => poisoned.into_inner().entries.clone(),
        }
    }

    fn entry(&self, entry_id: u64) -> Option<ClipboardHistoryItem> {
        match self.state.lock() {
            Ok(state) => state
                .entries
                .iter()
                .find(|entry| entry.id == entry_id)
                .cloned(),
            Err(poisoned) => poisoned
                .into_inner()
                .entries
                .iter()
                .find(|entry| entry.id == entry_id)
                .cloned(),
        }
    }

    fn record_text(&self, text: &str) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| "clipboard history lock poisoned".to_string())?;

        if state
            .entries
            .first()
            .is_some_and(|entry| entry.text == text)
        {
            return Ok(());
        }

        let entry = ClipboardHistoryItem {
            id: state.next_id,
            text: text.to_string(),
        };
        state.next_id += 1;
        state.entries.insert(0, entry);
        if state.entries.len() > self.max_entries {
            state.entries.truncate(self.max_entries);
        }

        save_state(&self.path, &state)
    }
}

#[derive(Debug, Clone)]
struct ClipboardHistoryState {
    next_id: u64,
    entries: Vec<ClipboardHistoryItem>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClipboardHistoryFile {
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    entries: Vec<ClipboardHistoryItem>,
}

fn load_state(path: &PathBuf, max_entries: usize) -> Result<ClipboardHistoryState, String> {
    let file = match std::fs::read(path) {
        Ok(contents) => serde_json::from_slice::<ClipboardHistoryFile>(&contents)
            .map_err(|error| format!("failed to parse clipboard history: {error}"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ClipboardHistoryFile {
            next_id: 1,
            entries: Vec::new(),
        },
        Err(error) => return Err(error.to_string()),
    };
    let ClipboardHistoryFile { next_id, entries } = file;

    let mut entries = entries
        .into_iter()
        .filter(|entry| !entry.text.trim().is_empty())
        .collect::<Vec<_>>();
    entries.truncate(max_entries);

    let next_id = entries
        .iter()
        .map(|entry| entry.id)
        .max()
        .map_or(next_id.max(1), |max_id| max_id.saturating_add(1));

    Ok(ClipboardHistoryState { next_id, entries })
}

fn save_state(path: &PathBuf, state: &ClipboardHistoryState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let contents = serde_json::to_vec_pretty(&ClipboardHistoryFile {
        next_id: state.next_id,
        entries: state.entries.clone(),
    })
    .map_err(|error| error.to_string())?;
    std::fs::write(path, contents).map_err(|error| error.to_string())
}

fn filter_entries(entries: &[ClipboardHistoryItem], query: &str) -> Vec<ClipboardHistoryItem> {
    let normalized_query = query.trim().to_lowercase();
    if normalized_query.is_empty() {
        return entries.to_vec();
    }

    entries
        .iter()
        .filter(|entry| entry.text.to_lowercase().contains(&normalized_query))
        .cloned()
        .collect()
}

fn to_session_result(entry: ClipboardHistoryItem) -> InteractiveSessionResult {
    let title = summarize_title(&entry.text);
    let subtitle = summarize_subtitle(&entry.text);

    InteractiveSessionResult {
        id: entry.id.to_string(),
        title,
        subtitle,
    }
}

fn summarize_title(text: &str) -> String {
    text.lines()
        .find(|line| !line.trim().is_empty())
        .map(truncate_preview)
        .unwrap_or_else(|| truncate_preview(text))
}

fn summarize_subtitle(text: &str) -> Option<String> {
    let mut lines = text.lines();
    let first_line = lines.next().unwrap_or_default();
    let remainder = lines.collect::<Vec<_>>().join(" ");

    if !remainder.trim().is_empty() {
        return Some(truncate_preview(&remainder));
    }

    if first_line.chars().count() > 80 {
        return Some(format!("{} chars", text.chars().count()));
    }

    None
}

fn truncate_preview(text: &str) -> String {
    const MAX_CHARS: usize = 80;

    let normalized = text.trim().replace('\t', " ");
    let mut chars = normalized.chars();
    let preview = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_core::CommandRegistry;
    use rayon_types::InteractiveSessionCompletionBehavior;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Default)]
    struct StubClipboardAccess {
        text: Mutex<Option<String>>,
        writes: Mutex<Vec<String>>,
    }

    impl ClipboardAccess for StubClipboardAccess {
        fn read_text(&self) -> Result<Option<String>, String> {
            Ok(self.text.lock().unwrap().clone())
        }

        fn write_text(&self, text: &str) -> Result<(), String> {
            self.writes.lock().unwrap().push(text.to_string());
            *self.text.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
    }

    fn temp_path(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rayon-clipboard-{test_name}-{unique}.json"))
    }

    fn service(path: PathBuf) -> (Arc<ClipboardHistoryService>, Arc<StubClipboardAccess>) {
        let access = Arc::new(StubClipboardAccess::default());
        let service = Arc::new(ClipboardHistoryService::new(access.clone(), path).unwrap());
        (service, access)
    }

    #[test]
    fn store_defaults_to_empty_when_missing() {
        let (service, _) = service(temp_path("missing"));
        assert!(service.recent_entries().is_empty());
    }

    #[test]
    fn store_persists_and_reloads_entries() {
        let path = temp_path("persist");
        let (service, _) = service(path.clone());
        service.record_text("first").unwrap();
        service.record_text("second").unwrap();

        let reloaded =
            ClipboardHistoryService::new(Arc::new(StubClipboardAccess::default()), path.clone())
                .unwrap();

        let entries = reloaded.recent_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "second");
        assert_eq!(entries[1].text, "first");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn store_caps_entries_and_skips_empty_values() {
        let (service, _) = service(temp_path("cap"));
        service.record_text("   ").unwrap();
        for index in 0..12 {
            service.record_text(&format!("item-{index}")).unwrap();
        }

        let entries = service.recent_entries();
        assert_eq!(entries.len(), 10);
        assert_eq!(entries[0].text, "item-11");
        assert_eq!(entries[9].text, "item-2");
    }

    #[test]
    fn store_dedupes_only_consecutive_values() {
        let (service, _) = service(temp_path("dedupe"));
        service.record_text("alpha").unwrap();
        service.record_text("alpha").unwrap();
        service.record_text("beta").unwrap();
        service.record_text("alpha").unwrap();

        let entries = service.recent_entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].text, "alpha");
        assert_eq!(entries[1].text, "beta");
        assert_eq!(entries[2].text, "alpha");
    }

    #[test]
    fn copy_entry_writes_to_clipboard_and_promotes_entry() {
        let (service, access) = service(temp_path("copy"));
        service.record_text("first").unwrap();
        service.record_text("second").unwrap();
        let first_entry_id = service.recent_entries()[1].id;

        let copied = service.copy_entry(first_entry_id).unwrap();

        assert_eq!(copied.text, "first");
        assert_eq!(access.writes.lock().unwrap().as_slice(), ["first"]);
        let entries = service.recent_entries();
        assert_eq!(entries[0].text, "first");
        assert_eq!(entries[1].text, "second");
    }

    #[test]
    fn provider_registers_and_filters_entries() {
        let (service, _) = service(temp_path("filter"));
        service.record_text("Deploy Rayon").unwrap();
        service.record_text("Clipboard notes").unwrap();

        let provider = ClipboardHistoryProvider::new(service.clone());
        let mut registry = CommandRegistry::new();
        registry
            .register_provider(Arc::new(ClipboardHistoryProvider::new(service.clone())))
            .unwrap();

        let results = registry.search_results_by_id();
        assert_eq!(results["clipboard"].title, "Clipboard History");

        let session = provider
            .start_interactive_session(&CommandId::from("clipboard"))
            .unwrap()
            .unwrap();
        assert_eq!(
            session.completion_behavior,
            InteractiveSessionCompletionBehavior::HideLauncherAndRestoreFocus
        );

        let filtered = provider
            .search_interactive_session(&session, "clip")
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Clipboard notes");
    }

    #[test]
    fn provider_search_and_submit_copy_selected_entry() {
        let (service, access) = service(temp_path("submit"));
        service.record_text("alpha").unwrap();
        service.record_text("beta").unwrap();
        let provider = ClipboardHistoryProvider::new(service.clone());
        let session = provider
            .start_interactive_session(&CommandId::from("clipboard"))
            .unwrap()
            .unwrap();

        let results = provider
            .search_interactive_session(&session, "alp")
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "alpha");

        let outcome = provider
            .submit_interactive_session(&session, "", &results[0].id)
            .unwrap();

        assert_eq!(
            outcome,
            InteractiveSessionSubmitOutcome::Completed(CommandExecutionResult {
                output: "copied clipboard item".into(),
            })
        );
        assert_eq!(access.writes.lock().unwrap().as_slice(), ["alpha"]);
    }
}
