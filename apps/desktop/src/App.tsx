import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import "./App.css";

type SearchResult = {
  id: string;
  title: string;
};

type CommandExecutionResult = {
  output: string;
};

function App() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [executionResult, setExecutionResult] = useState<string>("");
  const [error, setError] = useState<string>("");

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
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
  }, [query]);

  async function executeSelectedCommand() {
    const selectedCommand = results[selectedIndex];
    if (!selectedCommand) {
      return;
    }

    try {
      const response = await invoke<CommandExecutionResult>("execute_command", {
        commandId: selectedCommand.id,
        payload: null,
      });
      setExecutionResult(response.output);
      setError("");
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

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
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
        }
        break;
      default:
        break;
    }
  }

  return (
    <main className="app-shell">
      <section className="palette" aria-label="Command palette">
        <header className="palette-header">
          <p className="eyebrow">rayon</p>
          <h1>Command Palette</h1>
        </header>

        <input
          ref={inputRef}
          className="palette-input"
          value={query}
          onChange={(event) => setQuery(event.currentTarget.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a command"
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
        />

        <ul className="results" aria-label="Commands">
          {results.map((result, index) => (
            <li
              key={result.id}
              className={index === selectedIndex ? "result is-selected" : "result"}
              aria-selected={index === selectedIndex}
            >
              <span className="result-title">{result.title}</span>
              <span className="result-id">{result.id}</span>
            </li>
          ))}
          {results.length === 0 ? (
            <li className="result result-empty">No commands found.</li>
          ) : null}
        </ul>

        <section className="output" aria-live="polite">
          {executionResult ? <p>{executionResult}</p> : <p className="muted">Press Enter to execute.</p>}
          {error ? <p className="error">{error}</p> : null}
        </section>
      </section>
    </main>
  );
}

export default App;
