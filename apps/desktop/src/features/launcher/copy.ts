import type { InteractiveSessionState, SearchResult } from "@/commandExecution";

export function getInteractiveResultKind(session: InteractiveSessionState): string {
  if (session.command_id === "clipboard") {
    return "Clipboard";
  }
  if (session.command_id === "kill") {
    return "Process";
  }
  if (session.command_id === "github.my-prs") {
    return "Pull Request";
  }
  return "Option";
}

export function getInteractiveEmptyState(session: InteractiveSessionState): string {
  if (session.command_id === "clipboard") {
    return "No clipboard items found.";
  }
  if (session.command_id === "kill") {
    return "No matching processes.";
  }
  if (session.command_id === "github.my-prs") {
    return "No matching pull requests.";
  }
  return "No matching options.";
}

export function getInteractiveSubmitHint(session: InteractiveSessionState): string {
  if (session.command_id === "clipboard") {
    return "Press Enter to copy the selected item and close Rayon.";
  }
  if (session.command_id === "kill") {
    return "Press Enter to terminate the selected process.";
  }
  if (session.command_id === "github.my-prs") {
    return "Press Enter to open the selected pull request.";
  }
  return "Press Enter to continue.";
}

export function getSearchResultKind(result: SearchResult): string {
  if (result.kind === "browser_tab") {
    return "Tab";
  }
  if (result.kind === "application") {
    return "App";
  }
  if (result.arguments.length > 0) {
    return "Action";
  }
  return "Command";
}

export function getSearchResultMeta(result: SearchResult): string {
  return result.subtitle ?? result.owner_plugin_id ?? result.id;
}
