import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type KeyboardEvent, useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  beginPendingExecution,
  type CommandArgumentValue,
  type CommandInvocationResult,
  type InteractiveSessionState,
  type InteractiveSessionSubmitResult,
  currentArgument,
  currentArgumentInputValue,
  type PendingExecution,
  resolvePendingExecutionStep,
  type SearchResult,
} from "./commandExecution";
import "./App.css";
import { getLauncherViewState, shouldRunSearch } from "./launcherViewState";
import { applyThemePreference, type ThemePreference } from "./theme";

function App() {
  const shellRef = useRef<HTMLElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const resultItemRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const interactiveSearchRequestId = useRef(0);
  const optimisticSessionCounter = useRef(0);
  const requestedWindowHeight = useRef<number | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [interactiveSession, setInteractiveSession] = useState<InteractiveSessionState | null>(
    null,
  );
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [executionResult, setExecutionResult] = useState("");
  const [error, setError] = useState("");
  const [pendingExecution, setPendingExecution] = useState<PendingExecution | null>(null);
  const interactiveSessionId = interactiveSession?.session_id ?? null;
  const activeResults = interactiveSession ? interactiveSession.results : results;
  const selectedResultId = activeResults[selectedIndex]?.id ?? null;

  async function refreshThemePreference() {
    const theme = await invoke<ThemePreference>("get_theme_preference");
    applyThemePreference(theme);
  }

  function resetLauncher() {
    setQuery("");
    setExecutionResult("");
    setError("");
    setSelectedIndex(0);
    setPendingExecution(null);
    setInteractiveSession(null);
    setResults([]);
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
    let unlisten: (() => void) | undefined;

    async function bindLauncherOpenListener() {
      unlisten = await listen("launcher:opened", () => {
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
        const nextResults = await invoke<SearchResult[]>("search", { query });
        if (cancelled) {
          return;
        }

        setResults(nextResults);
        setSelectedIndex((currentIndex) => {
          if (nextResults.length === 0) {
            return 0;
          }

          return Math.min(currentIndex, nextResults.length - 1);
        });
        setError("");
      } catch (searchError) {
        if (cancelled) {
          return;
        }

        setResults([]);
        setSelectedIndex(0);
        setError(searchError instanceof Error ? searchError.message : String(searchError));
      }
    }

    void runSearch();

    return () => {
      cancelled = true;
    };
  }, [query, pendingExecution, interactiveSession]);

  useEffect(() => {
    if (!interactiveSessionId || interactiveSessionId.startsWith("pending:")) {
      return;
    }

    let cancelled = false;
    const requestId = interactiveSearchRequestId.current + 1;
    interactiveSearchRequestId.current = requestId;

    async function runInteractiveSearch() {
      try {
        const nextSession = await invoke<InteractiveSessionState>("search_interactive_session", {
          request: {
            session_id: interactiveSessionId,
            query,
          },
        });
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
          if (current?.session_id !== interactiveSessionId) {
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
    optimisticSessionId?: string,
  ) {
    try {
      const response = await invoke<CommandInvocationResult>("execute_command", {
        request: {
          command_id: commandId,
          arguments: argumentsMap,
        },
      });

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

  function handleQueryChange(nextQuery: string) {
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
      setSelectedIndex(0);
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
      setSelectedIndex(0);
      requestAnimationFrame(() => {
        inputRef.current?.focus();
      });
      await executeCommand(result.id, {}, optimisticSession.session_id);
      return;
    }

    await executeCommand(result.id, {});
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
      const response = await invoke<InteractiveSessionSubmitResult>("submit_interactive_session", {
        request: {
          session_id: interactiveSession.session_id,
          query,
          item_id: selectedResult.id,
        },
      });

      if (response.kind === "completed") {
        setExecutionResult(response.output);
        setError("");
        setPendingExecution(null);
        setInteractiveSession(null);
        setSelectedIndex(0);
        void refreshThemePreference();

        try {
          await invoke("hide_launcher");
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

  function interactiveResultKind(session: InteractiveSessionState): string {
    if (session.command_id === "kill") {
      return "Process";
    }
    if (session.command_id === "github.my-prs") {
      return "Pull Request";
    }
    return "Option";
  }

  function interactiveEmptyState(session: InteractiveSessionState): string {
    if (session.command_id === "kill") {
      return "No matching processes.";
    }
    if (session.command_id === "github.my-prs") {
      return "No matching pull requests.";
    }
    return "No matching options.";
  }

  function interactiveSubmitHint(session: InteractiveSessionState): string {
    if (session.command_id === "kill") {
      return "Press Enter to terminate the selected process.";
    }
    if (session.command_id === "github.my-prs") {
      return "Press Enter to open the selected pull request.";
    }
    return "Press Enter to continue.";
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

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
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

        void invoke("hide_launcher");
        break;
      default:
        break;
    }
  }

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

  useLayoutEffect(() => {
    if (!selectedResultId) {
      return;
    }

    resultItemRefs.current[selectedResultId]?.scrollIntoView({
      block: "nearest",
    });
  }, [selectedResultId]);

  useLayoutEffect(() => {
    const shell = shellRef.current;
    if (!shell) {
      return;
    }

    let frameId = 0;

    const syncLauncherHeight = () => {
      const measuredHeight = Math.ceil(shell.getBoundingClientRect().height);
      const nextHeight = Math.min(420, Math.max(160, measuredHeight));
      if (requestedWindowHeight.current === nextHeight) {
        return;
      }

      requestedWindowHeight.current = nextHeight;
      void invoke("resize_launcher", { height: nextHeight });
    };

    const scheduleSync = () => {
      cancelAnimationFrame(frameId);
      frameId = requestAnimationFrame(syncLauncherHeight);
    };

    scheduleSync();

    const observer = new ResizeObserver(() => {
      scheduleSync();
    });
    observer.observe(shell);

    return () => {
      cancelAnimationFrame(frameId);
      observer.disconnect();
    };
  }, []);

  return (
    <main ref={shellRef} className="launcher-shell">
      <section className="palette" aria-label="Command palette">
        {viewState.showHeader ? (
          <header className="palette-header">
            <p className="eyebrow">rayon</p>
            <h1>
              {pendingExecution
                ? pendingExecution.commandTitle
                : interactiveSession
                  ? interactiveSession.title
                  : "Command Palette"}
            </h1>
            {pendingExecution && activeArgument ? (
              <p className="arg-prompt">
                {activeArgument.label}
                {activeArgument.required ? " · required" : " · optional"}
              </p>
            ) : interactiveSession?.subtitle ? (
              <p className="arg-prompt">{interactiveSession.subtitle}</p>
            ) : null}
          </header>
        ) : null}

        <input
          ref={inputRef}
          className="palette-input"
          value={query}
          onChange={(event) => {
            handleQueryChange(event.currentTarget.value);
          }}
          onKeyDown={handleKeyDown}
          placeholder={
            activeArgument
              ? activeArgument.argument_type === "boolean"
                ? "true / false"
                : activeArgument.label
              : interactiveSession
                ? interactiveSession.input_placeholder
                : "Type a command"
          }
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
        />

        {pendingExecution ? (
          <section className="output output-arg" aria-live="polite">
            <p>
              Step {pendingExecution.currentIndex + 1} of {pendingExecution.arguments.length}
            </p>
            <p className="muted">
              {activeArgument?.flag ? `Flag ${activeArgument.flag}` : "Positional value"}
            </p>
            {activeArgument?.default_value ? (
              <p className="muted">Default: {currentArgumentInputValue(pendingExecution, query)}</p>
            ) : null}
          </section>
        ) : viewState.showResults ? (
          <ul className="results" aria-label="Search results">
            {showInteractiveSkeleton
              ? Array.from({ length: 6 }, (_, index) => (
                  <li key={`skeleton-${String(index)}`}>
                    <div className="result result-skeleton" aria-hidden="true">
                      <span className="result-copy">
                        <span className="result-row">
                          <span className="result-title-skeleton" />
                          <span className="result-kind-skeleton" />
                        </span>
                        <span className="result-meta-skeleton" />
                      </span>
                    </div>
                  </li>
                ))
              : interactiveSession
                ? interactiveSession.results.map((result, index) => (
                    <li key={result.id}>
                      <button
                        type="button"
                        className={index === selectedIndex ? "result is-selected" : "result"}
                        ref={(node) => {
                          resultItemRefs.current[result.id] = node;
                        }}
                        onMouseDown={(event) => {
                          event.preventDefault();
                        }}
                        onClick={() => {
                          setSelectedIndex(index);
                          void submitInteractiveSelection(result.id);
                        }}
                      >
                        <span className="result-copy">
                          <span className="result-row">
                            <span className="result-title">{result.title}</span>
                            <span className="result-kind">
                              {interactiveResultKind(interactiveSession)}
                            </span>
                          </span>
                          <span className="result-meta">{result.subtitle ?? result.id}</span>
                        </span>
                      </button>
                    </li>
                  ))
                : results.map((result, index) => (
                    <li key={result.id}>
                      <button
                        type="button"
                        className={index === selectedIndex ? "result is-selected" : "result"}
                        ref={(node) => {
                          resultItemRefs.current[result.id] = node;
                        }}
                        onMouseDown={(event) => {
                          event.preventDefault();
                        }}
                        onClick={() => {
                          setSelectedIndex(index);
                          void executeResult(result);
                        }}
                      >
                        <span className="result-copy">
                          <span className="result-row">
                            <span className="result-title">{result.title}</span>
                            <span className="result-kind">
                              {result.kind === "application"
                                ? "App"
                                : result.arguments.length > 0
                                  ? "Action"
                                  : "Command"}
                            </span>
                          </span>
                          <span className="result-meta">
                            {result.subtitle ?? result.owner_plugin_id ?? result.id}
                          </span>
                        </span>
                      </button>
                    </li>
                  ))}
            {viewState.showEmptyResults ? (
              <li className="result result-empty">No matches found.</li>
            ) : null}
            {interactiveSession &&
            !loadingInteractiveSession &&
            interactiveSession.results.length === 0 ? (
              <li className="result result-empty">{interactiveEmptyState(interactiveSession)}</li>
            ) : null}
          </ul>
        ) : null}

        {viewState.showFooter ? (
          <section className="output" aria-live="polite">
            {interactiveSession?.message ? (
              <p>{interactiveSession.message}</p>
            ) : executionResult ? (
              <p>{executionResult}</p>
            ) : pendingExecution ? (
              <p className="muted">Press Enter to continue.</p>
            ) : interactiveSession?.is_loading ? (
              <p className="muted">Loading results…</p>
            ) : interactiveSession ? (
              <p className="muted">{interactiveSubmitHint(interactiveSession)}</p>
            ) : null}
            {error ? <p className="error">{error}</p> : null}
          </section>
        ) : null}
      </section>
    </main>
  );
}

export default App;
