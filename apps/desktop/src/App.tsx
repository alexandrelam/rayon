import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import "./App.css";

type CommandArgumentType = "string" | "boolean";

type CommandArgumentDefinition = {
  id: string;
  label: string;
  argument_type: CommandArgumentType;
  required: boolean;
  flag: string | null;
  positional: number | null;
  default_value:
    | { type: "string"; value: string }
    | { type: "boolean"; value: boolean }
    | null;
};

type SearchResult = {
  id: string;
  title: string;
  subtitle: string | null;
  icon_path: string | null;
  kind: "command" | "application";
  owner_plugin_id: string | null;
  arguments: CommandArgumentDefinition[];
};

type CommandExecutionResult = {
  output: string;
};

type CommandArgumentValue =
  | { type: "string"; value: string }
  | { type: "boolean"; value: boolean };

type PendingExecution = {
  commandId: string;
  commandTitle: string;
  arguments: CommandArgumentDefinition[];
  values: Record<string, CommandArgumentValue>;
  currentIndex: number;
};

function App() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [executionResult, setExecutionResult] = useState<string>("");
  const [error, setError] = useState<string>("");
  const [pendingExecution, setPendingExecution] = useState<PendingExecution | null>(null);

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
    if (pendingExecution) {
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

  function currentArgument(): CommandArgumentDefinition | null {
    if (!pendingExecution) {
      return null;
    }

    return pendingExecution.arguments[pendingExecution.currentIndex] ?? null;
  }

  function currentArgumentInputValue(): string {
    const argument = currentArgument();
    if (!argument || !pendingExecution) {
      return query;
    }

    const currentValue = pendingExecution.values[argument.id];
    if (currentValue?.type === "string") {
      return currentValue.value;
    }
    if (currentValue?.type === "boolean") {
      return currentValue.value ? "true" : "false";
    }
    if (argument.default_value?.type === "string") {
      return argument.default_value.value;
    }
    if (argument.default_value?.type === "boolean") {
      return argument.default_value.value ? "true" : "false";
    }
    return "";
  }

  async function executeCommand(commandId: string, argumentsMap: Record<string, CommandArgumentValue>) {
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
      setQuery("");
      inputRef.current?.focus();
    } catch (executionError) {
      setExecutionResult("");
      setError(
        executionError instanceof Error
          ? executionError.message
          : String(executionError),
      );
    }
  }

  async function executeSelectedCommand() {
    const selectedCommand = results[selectedIndex];
    if (!selectedCommand) {
      return;
    }

    if (selectedCommand.kind === "command" && selectedCommand.arguments.length > 0) {
      setPendingExecution({
        commandId: selectedCommand.id,
        commandTitle: selectedCommand.title,
        arguments: selectedCommand.arguments,
        values: {},
        currentIndex: 0,
      });
      setQuery("");
      setExecutionResult("");
      setError("");
      return;
    }

    await executeCommand(selectedCommand.id, {});
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
    const argument = currentArgument();
    const activePendingExecution = pendingExecution;
    if (!argument || !activePendingExecution) {
      return;
    }

    const parsedValue = parseArgumentValue(argument, query);
    if (typeof parsedValue === "string") {
      setError(parsedValue);
      return;
    }

    const nextValues = { ...activePendingExecution.values };
    if (parsedValue) {
      nextValues[argument.id] = parsedValue;
    } else {
      delete nextValues[argument.id];
    }

    const nextIndex = activePendingExecution.currentIndex + 1;
    if (nextIndex >= activePendingExecution.arguments.length) {
      await executeCommand(activePendingExecution.commandId, nextValues);
      return;
    }

    setPendingExecution({
      ...activePendingExecution,
      values: nextValues,
      currentIndex: nextIndex,
    });
    setQuery("");
    setError("");
  }

  function parseArgumentValue(
    argument: CommandArgumentDefinition,
    rawValue: string,
  ): CommandArgumentValue | null | string {
    const trimmedValue = rawValue.trim();
    if (!trimmedValue) {
      if (argument.required && !argument.default_value) {
        return `${argument.label} is required`;
      }
      return null;
    }

    if (argument.argument_type === "string") {
      return { type: "string", value: trimmedValue };
    }

    const normalized = trimmedValue.toLowerCase();
    if (["true", "yes", "1", "on"].includes(normalized)) {
      return { type: "boolean", value: true };
    }
    if (["false", "no", "0", "off"].includes(normalized)) {
      return { type: "boolean", value: false };
    }

    return `${argument.label} expects true/false`;
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

  const activeArgument = currentArgument();

  return (
    <main className="launcher-shell">
      <section className="palette" aria-label="Command palette">
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

        <input
          ref={inputRef}
          className="palette-input"
          value={pendingExecution ? query : query}
          onChange={(event) => setQuery(event.currentTarget.value)}
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
              <p className="muted">Default: {currentArgumentInputValue()}</p>
            ) : null}
          </section>
        ) : (
          <ul className="results" aria-label="Search results">
            {results.map((result, index) => (
              <li
                key={result.id}
                className={index === selectedIndex ? "result is-selected" : "result"}
                aria-selected={index === selectedIndex}
              >
                <span className="result-copy">
                  <span className="result-row">
                    <span className="result-title">{result.title}</span>
                    <span className="result-kind">
                      {result.kind === "application" ? "App" : result.arguments.length > 0 ? "Action" : "Command"}
                    </span>
                  </span>
                    <span className="result-meta">
                    {result.subtitle ?? result.owner_plugin_id ?? result.id}
                  </span>
                </span>
              </li>
            ))}
            {results.length === 0 ? (
              <li className="result result-empty">No matches found.</li>
            ) : null}
          </ul>
        )}

        <section className="output" aria-live="polite">
          {executionResult ? <p>{executionResult}</p> : <p className="muted">{pendingExecution ? "Press Enter to continue." : "Press Enter to execute."}</p>}
          {error ? <p className="error">{error}</p> : null}
        </section>
      </section>
    </main>
  );
}

export default App;
