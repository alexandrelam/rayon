// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as tauriApp from "@tauri-apps/api/app";

import App from "./App";
import type {
  CommandInvocationResult,
  InteractiveSessionState,
  SearchResult,
} from "./commandExecution";
import * as launcherApi from "./features/launcher/api";

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(),
}));

vi.mock("./features/launcher/api", () => ({
  executeLauncherCommand: vi.fn(),
  hideLauncher: vi.fn(),
  refreshThemePreference: vi.fn(),
  registerLauncherOpenedListener: vi.fn(),
  resizeLauncher: vi.fn(),
  searchInteractiveSession: vi.fn(),
  searchLauncher: vi.fn(),
  submitLauncherInteractiveSelection: vi.fn(),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });

  return { promise, resolve };
}

const searchResult = (overrides: Partial<SearchResult> = {}): SearchResult => ({
  id: "echo",
  title: "Echo",
  subtitle: "Run echo",
  icon_path: null,
  kind: "command",
  owner_plugin_id: "user.commands",
  starts_interactive_session: false,
  arguments: [],
  ...overrides,
});

const interactiveSession = (
  overrides: Partial<InteractiveSessionState> = {},
): InteractiveSessionState => ({
  session_id: "session-1",
  command_id: "kill",
  title: "Kill Process",
  subtitle: "Search by app, process, or port",
  input_placeholder: "Search process name or port 8080",
  query: "",
  is_loading: false,
  results: [
    {
      id: "proc-1",
      title: "Rayon",
      subtitle: "PID 1234",
    },
  ],
  message: null,
  ...overrides,
});

describe("App", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    vi.clearAllMocks();

    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });

    Object.defineProperty(globalThis, "ResizeObserver", {
      configurable: true,
      value: class {
        observe = () => undefined;
        disconnect = () => undefined;
      },
    });

    Object.defineProperty(globalThis, "requestAnimationFrame", {
      configurable: true,
      value: (callback: FrameRequestCallback) => {
        callback(0);
        return 1;
      },
    });

    Object.defineProperty(globalThis, "cancelAnimationFrame", {
      configurable: true,
      value: vi.fn(),
    });

    vi.mocked(launcherApi.refreshThemePreference).mockResolvedValue(undefined);
    vi.mocked(launcherApi.registerLauncherOpenedListener).mockResolvedValue(vi.fn());
    vi.mocked(launcherApi.resizeLauncher).mockResolvedValue(undefined);
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([]);
    vi.mocked(launcherApi.searchInteractiveSession).mockResolvedValue(
      interactiveSession({ results: [] }),
    );
    vi.mocked(tauriApp.getVersion).mockResolvedValue("0.3.0");
    vi.mocked(launcherApi.executeLauncherCommand).mockResolvedValue({
      kind: "completed",
      output: "done",
    } satisfies CommandInvocationResult);
    vi.mocked(launcherApi.submitLauncherInteractiveSelection).mockResolvedValue({
      kind: "completed",
      output: "opened",
    });
    vi.mocked(launcherApi.hideLauncher).mockResolvedValue(undefined);
  });

  it("renders the idle launcher state", async () => {
    render(<App />);

    expect(await screen.findByText("Command Palette")).toBeTruthy();
    expect(await screen.findByText("v0.3.0")).toBeTruthy();
    expect(screen.getByLabelText("Command search").getAttribute("placeholder")).toBe(
      "Type a command",
    );
    expect(screen.queryByLabelText("Search results")).toBeNull();
  });

  it("renders without the version label when loading the app version fails", async () => {
    vi.mocked(tauriApp.getVersion).mockRejectedValue(new Error("failed to read app version"));

    render(<App />);

    expect(await screen.findByText("Command Palette")).toBeTruthy();
    await waitFor(() => {
      expect(tauriApp.getVersion).toHaveBeenCalled();
    });
    expect(screen.queryByText("v0.3.0")).toBeNull();
  });

  it("navigates search results with the keyboard and executes the selected command", async () => {
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({ id: "alpha", title: "Alpha", subtitle: "First" }),
      searchResult({ id: "beta", title: "Beta", subtitle: "Second" }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "a" } });

    expect(await screen.findByText("Alpha")).toBeTruthy();

    await userEvent.click(input);
    await userEvent.keyboard("{ArrowUp}{Enter}");

    await waitFor(() => {
      expect(launcherApi.executeLauncherCommand).toHaveBeenCalledWith("beta", {});
    });
  });

  it("validates pending argument entry before executing the command", async () => {
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({
        id: "apps.reindex",
        title: "Reindex Search",
        arguments: [
          {
            id: "enabled",
            label: "Enabled",
            argument_type: "boolean",
            required: true,
            flag: "--enabled",
            positional: null,
            default_value: null,
          },
        ],
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "reindex" } });

    expect(await screen.findByText("Reindex Search")).toBeTruthy();

    await userEvent.keyboard("{Enter}");

    expect(await screen.findByText("Step 1 of 1")).toBeTruthy();
    expect(input.getAttribute("placeholder")).toBe("true / false");

    fireEvent.change(input, { target: { value: "maybe" } });
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByText("Enabled expects true/false")).toBeTruthy();

    fireEvent.change(input, { target: { value: "true" } });
    await userEvent.keyboard("{Enter}");

    await waitFor(() => {
      expect(launcherApi.executeLauncherCommand).toHaveBeenCalledWith("apps.reindex", {
        enabled: {
          type: "boolean",
          value: true,
        },
      });
    });
  });

  it("shows the interactive loading state and submits the selected item", async () => {
    const pendingExecution = deferred<CommandInvocationResult>();
    const session = interactiveSession();

    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({
        id: "kill",
        title: "Kill Process",
        subtitle: "Search by app, process, or port",
        starts_interactive_session: true,
      }),
    ]);
    vi.mocked(launcherApi.executeLauncherCommand).mockReturnValue(pendingExecution.promise);
    vi.mocked(launcherApi.searchInteractiveSession).mockResolvedValue(session);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "kill" } });

    expect(await screen.findByText("Kill Process")).toBeTruthy();
    await userEvent.keyboard("{Enter}");

    expect(await screen.findByText("Loading results…")).toBeTruthy();

    pendingExecution.resolve({
      kind: "started_session",
      session,
    });

    expect(await screen.findByText("Rayon")).toBeTruthy();

    await userEvent.keyboard("{Enter}");

    await waitFor(() => {
      expect(launcherApi.submitLauncherInteractiveSelection).toHaveBeenCalledWith(
        "session-1",
        "",
        "proc-1",
      );
    });
    await waitFor(() => {
      expect(launcherApi.hideLauncher).toHaveBeenCalled();
    });
  });
});
