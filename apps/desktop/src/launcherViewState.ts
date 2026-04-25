import type { InteractiveSessionState, PendingExecution, SearchResult } from "./commandExecution";

type LauncherViewStateInput = {
  query: string;
  results: SearchResult[];
  executionResult: string;
  error: string;
  pendingExecution: PendingExecution | null;
  interactiveSession: InteractiveSessionState | null;
};

export function isLauncherIdle({
  query,
  executionResult,
  error,
  pendingExecution,
  interactiveSession,
}: Omit<LauncherViewStateInput, "results">): boolean {
  return (
    query === "" &&
    executionResult === "" &&
    error === "" &&
    pendingExecution === null &&
    interactiveSession === null
  );
}

export function shouldRunSearch({
  query,
  pendingExecution,
  interactiveSession,
}: Pick<LauncherViewStateInput, "query" | "pendingExecution" | "interactiveSession">): boolean {
  return pendingExecution === null && interactiveSession === null && query !== "";
}

export function getLauncherViewState(input: LauncherViewStateInput) {
  const idle = isLauncherIdle(input);
  const showingArgumentPrompt = input.pendingExecution !== null;
  const showingInteractiveSession = input.interactiveSession !== null;
  const showFooter =
    showingArgumentPrompt ||
    showingInteractiveSession ||
    input.executionResult !== "" ||
    input.error !== "";

  return {
    idle,
    showHeader: true,
    showResults: !idle && !showingArgumentPrompt,
    showEmptyResults:
      !idle && !showingArgumentPrompt && input.results.length === 0 && !showingInteractiveSession,
    showFooter,
    showingArgumentPrompt,
    showingInteractiveSession,
  };
}
