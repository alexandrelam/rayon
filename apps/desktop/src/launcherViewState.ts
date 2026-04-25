import type { PendingExecution, SearchResult } from "./commandExecution";

type LauncherViewStateInput = {
  query: string;
  results: SearchResult[];
  executionResult: string;
  error: string;
  pendingExecution: PendingExecution | null;
};

export function isLauncherIdle({
  query,
  executionResult,
  error,
  pendingExecution,
}: Omit<LauncherViewStateInput, "results">): boolean {
  return query === "" && executionResult === "" && error === "" && pendingExecution === null;
}

export function shouldRunSearch({
  query,
  pendingExecution,
}: Pick<LauncherViewStateInput, "query" | "pendingExecution">): boolean {
  return pendingExecution === null && query !== "";
}

export function getLauncherViewState(input: LauncherViewStateInput) {
  const idle = isLauncherIdle(input);
  const showingArgumentPrompt = input.pendingExecution !== null;
  const showFooter = showingArgumentPrompt || input.executionResult !== "" || input.error !== "";

  return {
    idle,
    showHeader: true,
    showResults: !idle && !showingArgumentPrompt,
    showEmptyResults: !idle && !showingArgumentPrompt && input.results.length === 0,
    showFooter,
    showingArgumentPrompt,
  };
}
