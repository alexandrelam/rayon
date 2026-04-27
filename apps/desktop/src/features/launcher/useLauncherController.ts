import { type KeyboardEventHandler, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

import {
  beginPendingExecution,
  type CommandArgumentValue,
  currentArgument,
  currentArgumentInputValue,
  type InteractiveSessionState,
  matchesExactKeywordAlias,
  parseCommandLine,
  type PendingExecution,
  resolveInlineArgv,
  resolvePendingExecutionStep,
  scheduleAfterNextPaint,
  type SearchResult,
} from "@/commandExecution";
import { getLauncherViewState, shouldRunSearch } from "@/launcherViewState";
import { applyThemePreference } from "@/theme";

import {
  executeLauncherCommand,
  hideLauncher,
  refreshThemePreference,
  registerLauncherOpenedListener,
  searchInteractiveSession,
  searchLauncher,
  submitLauncherInteractiveSelection,
} from "./api";
import {
  getInteractiveEmptyState,
  getInteractiveResultKind,
  getInteractiveSubmitHint,
  getSearchResultKind,
  getSearchResultMeta,
} from "./copy";
import { useLauncherAutoResize } from "./hooks/useLauncherAutoResize";
import { useSelectedResultScroll } from "./hooks/useSelectedResultScroll";
import type {
  LauncherArgumentPanelViewModel,
  LauncherController,
  LauncherFooterViewModel,
  LauncherHeaderViewModel,
  LauncherResultItemViewModel,
} from "./types";

export function useLauncherController(): LauncherController {
  const shellRef = useRef<HTMLElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const resultItemRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const interactiveSearchRequestId = useRef(0);
  const optimisticSessionCounter = useRef(0);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [interactiveSession, setInteractiveSession] = useState<InteractiveSessionState | null>(
    null,
  );
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [executionResult, setExecutionResult] = useState("");
  const [error, setError] = useState("");
  const [pendingExecution, setPendingExecution] = useState<PendingExecution | null>(null);
  const [inlineFallbackArgv, setInlineFallbackArgv] = useState<string[]>([]);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const interactiveSessionId = interactiveSession?.session_id ?? null;
  const activeResults = interactiveSession ? interactiveSession.results : results;
  const selectedResultId = activeResults[selectedIndex]?.id ?? null;

  useSelectedResultScroll(selectedResultId, resultItemRefs);
  useLauncherAutoResize(shellRef);

  function resetLauncher() {
    setQuery("");
    setExecutionResult("");
    setError("");
    setSelectedIndex(0);
    setPendingExecution(null);
    setInteractiveSession(null);
    setResults([]);
    setInlineFallbackArgv([]);
    requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    });
  }

  useEffect(() => {
    inputRef.current?.focus();
    applyThemePreference("system");
    void refreshThemePreference();
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function loadAppVersion() {
      try {
        const version = await getVersion();
        if (!cancelled) {
          setAppVersion(version);
        }
      } catch {
        if (!cancelled) {
          setAppVersion(null);
        }
      }
    }

    void loadAppVersion();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    async function bindLauncherOpenListener() {
      unlisten = await registerLauncherOpenedListener(() => {
        resetLauncher();
      });
    }

    void bindLauncherOpenListener();

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!shouldRunSearch({ query, pendingExecution, interactiveSession })) {
      return;
    }

    let cancelled = false;

    async function runSearch() {
      try {
        const nextResults = await resolveLauncherSearch(query);
        if (cancelled) {
          return;
        }

        setResults(nextResults.results);
        setInlineFallbackArgv(nextResults.inlineFallbackArgv);
        setSelectedIndex((currentIndex) => {
          if (nextResults.results.length === 0) {
            return 0;
          }

          return Math.min(currentIndex, nextResults.results.length - 1);
        });
        setError("");
      } catch (searchError) {
        if (cancelled) {
          return;
        }

        setResults([]);
        setInlineFallbackArgv([]);
        setSelectedIndex(0);
        setError(searchError instanceof Error ? searchError.message : String(searchError));
      }
    }

    void runSearch();

    return () => {
      cancelled = true;
    };
  }, [interactiveSession, pendingExecution, query]);

  useEffect(() => {
    if (!interactiveSessionId || interactiveSessionId.startsWith("pending:")) {
      return;
    }

    const sessionId = interactiveSessionId;
    let cancelled = false;
    const requestId = interactiveSearchRequestId.current + 1;
    interactiveSearchRequestId.current = requestId;

    async function runInteractiveSearch() {
      try {
        const nextSession = await searchInteractiveSession(sessionId, query);
        if (cancelled || interactiveSearchRequestId.current !== requestId) {
          return;
        }

        setInteractiveSession(nextSession);
        setSelectedIndex((currentIndex) => {
          if (nextSession.results.length === 0) {
            return 0;
          }

          return Math.min(currentIndex, nextSession.results.length - 1);
        });
        setError("");
      } catch (searchError) {
        if (cancelled || interactiveSearchRequestId.current !== requestId) {
          return;
        }

        setInteractiveSession((current) => {
          if (current?.session_id !== sessionId) {
            return current;
          }

          return {
            ...current,
            query,
            is_loading: false,
          };
        });
        setError(searchError instanceof Error ? searchError.message : String(searchError));
      }
    }

    void runInteractiveSearch();

    return () => {
      cancelled = true;
    };
  }, [interactiveSessionId, query]);

  async function executeCommand(
    commandId: string,
    argumentsMap: Record<string, CommandArgumentValue>,
    argv: string[] = [],
    optimisticSessionId?: string,
  ) {
    try {
      const response = await executeLauncherCommand(commandId, argumentsMap, argv);

      if (response.kind === "started_session") {
        setInteractiveSession(response.session);
        setQuery(response.session.query);
        setResults([]);
        setExecutionResult("");
        setError("");
        setPendingExecution(null);
        setSelectedIndex(0);
        void refreshThemePreference();
        return;
      }

      setExecutionResult(response.output);
      setError("");
      setPendingExecution(null);
      setInteractiveSession(null);
      inputRef.current?.focus();
      void refreshThemePreference();
    } catch (executionError) {
      if (optimisticSessionId) {
        setInteractiveSession((current) =>
          current?.session_id === optimisticSessionId ? null : current,
        );
      }
      setExecutionResult("");
      setError(executionError instanceof Error ? executionError.message : String(executionError));
    }
  }

  function createOptimisticInteractiveSession(result: SearchResult): InteractiveSessionState {
    optimisticSessionCounter.current += 1;
    return {
      session_id: `pending:${result.id}:${String(optimisticSessionCounter.current)}`,
      command_id: result.id,
      title: result.title,
      subtitle: result.subtitle,
      input_placeholder: "Type to filter",
      query: "",
      is_loading: true,
      results: [],
      message: null,
    };
  }

  async function executeResult(result: SearchResult) {
    const nextPendingExecution = beginPendingExecution(result);
    if (nextPendingExecution) {
      setPendingExecution(nextPendingExecution);
      setQuery("");
      setExecutionResult("");
      setError("");
      setResults([]);
      setInlineFallbackArgv([]);
      return;
    }

    const inlineExecution = resolveInlineArgv(result, query, inlineFallbackArgv);
    if (inlineExecution.kind === "error") {
      setExecutionResult("");
      setError(inlineExecution.message);
      return;
    }

    if (result.kind === "command" && result.starts_interactive_session) {
      const optimisticSession = createOptimisticInteractiveSession(result);
      setInteractiveSession(optimisticSession);
      setQuery("");
      setResults([]);
      setExecutionResult("");
      setError("");
      setPendingExecution(null);
      setInlineFallbackArgv([]);
      setSelectedIndex(0);
      requestAnimationFrame(() => {
        inputRef.current?.focus();
      });
      scheduleAfterNextPaint(() => {
        void executeCommand(result.id, {}, inlineExecution.argv, optimisticSession.session_id);
      });
      return;
    }

    await executeCommand(result.id, {}, inlineExecution.argv);
  }

  async function executeSelectedCommand() {
    if (selectedIndex >= results.length) {
      return;
    }

    const selectedCommand = results[selectedIndex];
    await executeResult(selectedCommand);
  }

  async function submitInteractiveSelection(itemId?: string) {
    if (!interactiveSession || interactiveSession.results.length === 0) {
      return;
    }

    const selectedResult =
      interactiveSession.results.find((result) => result.id === itemId) ??
      interactiveSession.results[selectedIndex];

    try {
      const response = await submitLauncherInteractiveSelection(
        interactiveSession.session_id,
        query,
        selectedResult.id,
      );

      if (response.kind === "completed") {
        setExecutionResult(response.output);
        setError("");
        setPendingExecution(null);
        setInteractiveSession(null);
        setSelectedIndex(0);
        void refreshThemePreference();

        try {
          await hideLauncher();
          resetLauncher();
        } catch (hideError) {
          setError(hideError instanceof Error ? hideError.message : String(hideError));
        }
        return;
      }

      const nextSession = response.session;
      setInteractiveSession(nextSession);
      setSelectedIndex((currentIndex) => {
        if (nextSession.results.length === 0) {
          return 0;
        }

        return Math.min(currentIndex, nextSession.results.length - 1);
      });
      setExecutionResult("");
      setError("");
      void refreshThemePreference();
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : String(submitError));
    }
  }

  function moveSelection(direction: -1 | 1) {
    setSelectedIndex((currentIndex) => {
      if (activeResults.length === 0) {
        return 0;
      }

      const nextIndex = currentIndex + direction;
      if (nextIndex < 0) {
        return activeResults.length - 1;
      }

      if (nextIndex >= activeResults.length) {
        return 0;
      }

      return nextIndex;
    });
  }

  async function submitArgumentValue() {
    const activePendingExecution = pendingExecution;
    if (!activePendingExecution) {
      return;
    }

    const step = resolvePendingExecutionStep(activePendingExecution, query);
    if (step.kind === "error") {
      setError(step.message);
      return;
    }

    if (step.kind === "advance") {
      setPendingExecution(step.pendingExecution);
      setQuery("");
      setError("");
      setResults([]);
      return;
    }

    await executeCommand(step.commandId, step.argumentsMap);
  }

  function onQueryChange(nextQuery: string) {
    setQuery(nextQuery);
    setError("");

    if (interactiveSession) {
      setInteractiveSession({
        ...interactiveSession,
        query: nextQuery,
        is_loading: true,
        message: null,
      });
      return;
    }

    if (!pendingExecution && nextQuery !== "") {
      setExecutionResult("");
    }

    if (!pendingExecution && nextQuery === "") {
      setResults([]);
      setInlineFallbackArgv([]);
      setSelectedIndex(0);
    }
  }

  const onKeyDown: KeyboardEventHandler<HTMLInputElement> = (event) => {
    if (pendingExecution) {
      switch (event.key) {
        case "Enter":
          event.preventDefault();
          void submitArgumentValue();
          break;
        case "Escape":
          event.preventDefault();
          if (pendingExecution.currentIndex === 0) {
            setPendingExecution(null);
            setQuery("");
            setError("");
            setResults([]);
            return;
          }

          setPendingExecution((current) => {
            if (!current) {
              return current;
            }

            return {
              ...current,
              currentIndex: current.currentIndex - 1,
            };
          });
          setQuery("");
          setError("");
          break;
        default:
          break;
      }
      return;
    }

    if (interactiveSession) {
      switch (event.key) {
        case "ArrowDown":
          event.preventDefault();
          moveSelection(1);
          break;
        case "ArrowUp":
          event.preventDefault();
          moveSelection(-1);
          break;
        case "Enter":
          event.preventDefault();
          void submitInteractiveSelection();
          break;
        case "Escape":
          event.preventDefault();
          setInteractiveSession(null);
          setQuery("");
          setError("");
          setSelectedIndex(0);
          return;
        default:
          break;
      }
      return;
    }

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        moveSelection(1);
        break;
      case "ArrowUp":
        event.preventDefault();
        moveSelection(-1);
        break;
      case "Enter":
        event.preventDefault();
        void executeSelectedCommand();
        break;
      case "Escape":
        event.preventDefault();
        if (executionResult) {
          setExecutionResult("");
          return;
        }

        if (query) {
          setQuery("");
          setError("");
          setSelectedIndex(0);
          return;
        }

        void hideLauncher();
        break;
      default:
        break;
    }
  };

  const activeArgument = currentArgument(pendingExecution);
  const viewState = getLauncherViewState({
    query,
    results,
    executionResult,
    error,
    pendingExecution,
    interactiveSession,
  });
  const loadingInteractiveSession = interactiveSession?.is_loading ?? false;
  const showInteractiveSkeleton =
    interactiveSession !== null &&
    loadingInteractiveSession &&
    interactiveSession.results.length === 0;
  const selectedSearchResult = interactiveSession ? null : (results[selectedIndex] ?? null);

  const resultItems: LauncherResultItemViewModel[] = interactiveSession
    ? interactiveSession.results.map((result, index) => ({
        id: result.id,
        title: result.title,
        meta: result.subtitle ?? result.id,
        kind: getInteractiveResultKind(interactiveSession),
        selected: index === selectedIndex,
      }))
    : results.map((result, index) => ({
        id: result.id,
        title: result.title,
        meta: getSearchResultMeta(result),
        kind: getSearchResultKind(result),
        selected: index === selectedIndex,
      }));

  const header: LauncherHeaderViewModel = {
    title: pendingExecution
      ? pendingExecution.commandTitle
      : interactiveSession
        ? interactiveSession.title
        : "Command Palette",
    subtitle:
      pendingExecution && activeArgument
        ? `${activeArgument.label}${activeArgument.required ? " · required" : " · optional"}`
        : (interactiveSession?.subtitle ?? null),
    version: appVersion,
  };

  const argumentPanel: LauncherArgumentPanelViewModel | null =
    pendingExecution && activeArgument
      ? {
          currentStep: pendingExecution.currentIndex + 1,
          totalSteps: pendingExecution.arguments.length,
          flagLabel: activeArgument.flag ? `Flag ${activeArgument.flag}` : "Positional value",
          defaultValue: activeArgument.default_value
            ? currentArgumentInputValue(pendingExecution, query)
            : null,
        }
      : null;

  const footer: LauncherFooterViewModel = interactiveSession?.message
    ? {
        message: interactiveSession.message,
        muted: false,
        error,
      }
    : executionResult
      ? {
          message: executionResult,
          muted: false,
          error,
        }
      : pendingExecution
        ? {
            message: "Press Enter to continue.",
            muted: true,
            error,
          }
        : selectedSearchResult?.kind === "command" && selectedSearchResult.input_mode === "raw_argv"
          ? {
              message:
                inlineFallbackArgv.length > 0
                  ? `Press Enter to run with ${String(inlineFallbackArgv.length)} argument${inlineFallbackArgv.length === 1 ? "" : "s"}.`
                  : "Press Enter to run.",
              muted: true,
              error,
            }
          : interactiveSession?.is_loading
            ? {
                message: "Loading results…",
                muted: true,
                error,
              }
            : interactiveSession
              ? {
                  message: getInteractiveSubmitHint(interactiveSession),
                  muted: true,
                  error,
                }
              : {
                  message: null,
                  muted: true,
                  error,
                };

  let emptyMessage: string | null = null;
  if (viewState.showEmptyResults) {
    emptyMessage = "No matches found.";
  } else if (
    interactiveSession &&
    !loadingInteractiveSession &&
    interactiveSession.results.length === 0
  ) {
    emptyMessage = getInteractiveEmptyState(interactiveSession);
  }

  return {
    shellRef,
    inputRef,
    query,
    placeholder: activeArgument
      ? activeArgument.argument_type === "boolean"
        ? "true / false"
        : activeArgument.label
      : interactiveSession
        ? interactiveSession.input_placeholder
        : "Type a command",
    onQueryChange,
    onKeyDown,
    header,
    showHeader: viewState.showHeader,
    showResults: viewState.showResults,
    showFooter: viewState.showFooter,
    resultItems,
    showInteractiveSkeleton,
    emptyMessage,
    footer,
    argumentPanel,
    onResultSelect: (itemId) => {
      const nextIndex = activeResults.findIndex((result) => result.id === itemId);
      if (nextIndex < 0) {
        return;
      }

      setSelectedIndex(nextIndex);

      if (interactiveSession) {
        void submitInteractiveSelection(itemId);
        return;
      }

      const selectedItem = results[nextIndex];
      void executeResult(selectedItem);
    },
    setResultItemRef: (itemId, node) => {
      resultItemRefs.current[itemId] = node;
    },
    pendingExecution,
  };
}

async function resolveLauncherSearch(query: string): Promise<{
  results: SearchResult[];
  inlineFallbackArgv: string[];
}> {
  const directResults = await searchLauncher(query);
  const parsed = parseCommandLine(query);
  if (parsed.kind === "error" || parsed.tokens.length < 2) {
    return {
      results: directResults,
      inlineFallbackArgv: [],
    };
  }

  for (let prefixLength = parsed.tokens.length - 1; prefixLength > 0; prefixLength -= 1) {
    const prefixTokens = parsed.tokens.slice(0, prefixLength);
    const hasDirectAliasMatch = directResults.some((result) => {
      return matchesExactKeywordAlias(result, prefixTokens);
    });
    if (hasDirectAliasMatch) {
      return {
        results: directResults,
        inlineFallbackArgv: parsed.tokens.slice(prefixLength),
      };
    }
  }

  for (let prefixLength = parsed.tokens.length - 1; prefixLength > 0; prefixLength -= 1) {
    const prefixQuery = parsed.tokens.slice(0, prefixLength).join(" ");
    const prefixResults = await searchLauncher(prefixQuery);
    const hasExactAliasMatch = prefixResults.some((result) => {
      return matchesExactKeywordAlias(result, parsed.tokens.slice(0, prefixLength));
    });
    if (hasExactAliasMatch) {
      return {
        results: prefixResults,
        inlineFallbackArgv: parsed.tokens.slice(prefixLength),
      };
    }
  }

  return {
    results: directResults,
    inlineFallbackArgv: [],
  };
}
