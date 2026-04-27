import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  CommandArgumentValue,
  CommandInvocationResult,
  InteractiveSessionState,
  InteractiveSessionSubmitResult,
  SearchResult,
} from "@/commandExecution";
import { applyThemePreference, type ThemePreference } from "@/theme";

export async function refreshThemePreference() {
  const theme = await invoke<ThemePreference>("get_theme_preference");
  applyThemePreference(theme);
}

export async function registerLauncherOpenedListener(onOpen: () => void) {
  return listen("launcher:opened", () => {
    onOpen();
  });
}

export async function searchLauncher(query: string) {
  return invoke<SearchResult[]>("search", { query });
}

export async function searchBrowserTabs(query: string, refresh = false) {
  return invoke<SearchResult[]>("search_browser_tabs", { query, refresh });
}

export async function searchInteractiveSession(sessionId: string, query: string) {
  return invoke<InteractiveSessionState>("search_interactive_session", {
    request: {
      session_id: sessionId,
      query,
    },
  });
}

export async function executeLauncherCommand(
  commandId: string,
  argumentsMap: Record<string, CommandArgumentValue>,
  argv: string[] = [],
) {
  return invoke<CommandInvocationResult>("execute_command", {
    request: {
      command_id: commandId,
      argv,
      arguments: argumentsMap,
    },
  });
}

export async function submitLauncherInteractiveSelection(
  sessionId: string,
  query: string,
  itemId: string,
) {
  return invoke<InteractiveSessionSubmitResult>("submit_interactive_session", {
    request: {
      session_id: sessionId,
      query,
      item_id: itemId,
    },
  });
}

export async function hideLauncher() {
  await invoke("hide_launcher");
}

export async function hideLauncherAndRestoreFocus() {
  await invoke("hide_launcher_and_restore_focus");
}

export async function resizeLauncher(height: number) {
  await invoke("resize_launcher", { height });
}
