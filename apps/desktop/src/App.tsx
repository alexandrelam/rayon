import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import {
  beginPendingExecution,
  type CommandArgumentValue,
  type CommandExecutionResult,
  currentArgument,
  currentArgumentInputValue,
  type PendingExecution,
  resolvePendingExecutionStep,
  type SearchResult,
} from "./commandExecution";
import "./App.css";
import { getLauncherViewState, shouldRunSearch } from "./launcherViewState";

function App() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [executionResult, setExecutionResult] = useState<string>("");
  const [error, setError] = useState<string>("");
  const [pendingExecution, setPendingExecution] = useState<PendingExecution | null>(null);

  function resetLauncher() {
    setQuery("");
    setExecutionResult("");
    setError("");
    setSelectedIndex(0);
    setPendingExecution(null);
    setResults([]);
    requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    });
  }

  useEffect(() => {
    inputRef.current?.focus();
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
    if (!shouldRunSearch({ query, pendingExecution })) {
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
  }, [query, pendingExecution]);

  async function executeCommand(
    commandId: string,
    argumentsMap: Record<string, CommandArgumentValue>,
  ) {
    try {
      const response = await invoke<CommandExecutionResult>("execute_command", {
        request: {
          command_id: commandId,
          arguments: argumentsMap,
        },
      });
      setExecutionResult(response.output);
      setError("");
      setPendingExecution(null);
      inputRef.current?.focus();
    } catch (executionError) {
      setExecutionResult("");
      setError(executionError instanceof Error ? executionError.message : String(executionError));
    }
  }

  function handleQueryChange(nextQuery: string) {
    setQuery(nextQuery);
    setError("");

    if (!pendingExecution && nextQuery !== "") {
      setExecutionResult("");
    }

    if (!pendingExecution && nextQuery === "") {
      setResults([]);
      setSelectedIndex(0);
    }
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

    await executeCommand(result.id, {});
  }

  async function executeSelectedCommand() {
    if (selectedIndex >= results.length) {
      return;
    }

    const selectedCommand = results[selectedIndex];
    await executeResult(selectedCommand);
  }

  function moveSelection(direction: -1 | 1) {
    setSelectedIndex((currentIndex) => {
      if (results.length === 0) {
        return 0;
      }

      const nextIndex = currentIndex + direction;
      if (nextIndex < 0) {
        return results.length - 1;
      }

      if (nextIndex >= results.length) {
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
  });

  return (
    <main className="launcher-shell">
      <section className="palette" aria-label="Command palette">
        {viewState.showHeader ? (
          <header className="palette-header">
            <p className="eyebrow">rayon</p>
            <h1>{pendingExecution ? pendingExecution.commandTitle : "Command Palette"}</h1>
            {pendingExecution && activeArgument ? (
              <p className="arg-prompt">
                {activeArgument.label}
                {activeArgument.required ? " · required" : " · optional"}
              </p>
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
            {results.map((result, index) => (
              <li key={result.id}>
                <button
                  type="button"
                  className={index === selectedIndex ? "result is-selected" : "result"}
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
          </ul>
        ) : null}

        {viewState.showFooter ? (
          <section className="output" aria-live="polite">
            {executionResult ? (
              <p>{executionResult}</p>
            ) : pendingExecution ? (
              <p className="muted">Press Enter to continue.</p>
            ) : null}
            {error ? <p className="error">{error}</p> : null}
          </section>
        ) : null}
      </section>
    </main>
  );
}

export default App;
