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
  hideLauncherAndRestoreFocus: vi.fn(),
  refreshThemePreference: vi.fn(),
  registerLauncherOpenedListener: vi.fn(),
  resizeLauncher: vi.fn(),
  searchBrowserTabs: vi.fn(),
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
  keywords: [],
  starts_interactive_session: false,
  close_launcher_on_success: false,
  input_mode: "structured",
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
  completion_behavior: "hide_launcher",
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
    vi.mocked(launcherApi.searchBrowserTabs).mockResolvedValue([]);
    vi.mocked(launcherApi.searchInteractiveSession).mockResolvedValue(
      interactiveSession({ results: [] }),
    );
    vi.mocked(tauriApp.getVersion).mockResolvedValue("0.4.0");
    vi.mocked(launcherApi.executeLauncherCommand).mockResolvedValue({
      kind: "completed",
      output: "done",
    } satisfies CommandInvocationResult);
    vi.mocked(launcherApi.submitLauncherInteractiveSelection).mockResolvedValue({
      kind: "completed",
      output: "opened",
      completion_behavior: "hide_launcher",
    });
    vi.mocked(launcherApi.hideLauncher).mockResolvedValue(undefined);
    vi.mocked(launcherApi.hideLauncherAndRestoreFocus).mockResolvedValue(undefined);
  });

  it("renders the idle launcher state", async () => {
    render(<App />);

    expect(await screen.findByText("Command Palette")).toBeTruthy();
    expect(await screen.findByText("v0.4.0")).toBeTruthy();
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
    expect(screen.queryByText("v0.4.0")).toBeNull();
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
      expect(launcherApi.executeLauncherCommand).toHaveBeenCalledWith("beta", {}, []);
    });
  });

  it("uses the default launcher search when the query does not start with a space", async () => {
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({ id: "alpha", title: "Alpha" }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "alpha" } });

    expect(await screen.findByText("Alpha")).toBeTruthy();
    expect(launcherApi.searchLauncher).toHaveBeenCalledWith("alpha");
    expect(launcherApi.searchBrowserTabs).not.toHaveBeenCalled();
    expect(input.getAttribute("placeholder")).toBe("Type a command");
    expect(input.getAttribute("data-mode")).toBe("default");
  });

  it("uses browser tab search when the query starts with a space", async () => {
    vi.mocked(launcherApi.searchBrowserTabs).mockResolvedValue([
      searchResult({
        id: "browser-tab:chrome:window-1:1",
        title: "Issue 15",
        subtitle: "github.com",
        kind: "browser_tab",
        close_launcher_on_success: true,
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: " issue" } });

    expect(await screen.findByText("Issue 15")).toBeTruthy();
    expect(launcherApi.searchBrowserTabs).toHaveBeenCalledWith("issue", true);
    expect(launcherApi.searchLauncher).not.toHaveBeenCalled();
    expect(input.getAttribute("placeholder")).toBe("Search open windows and tabs");
    expect(input.getAttribute("data-mode")).toBe("browser_tabs");
  });

  it("renders image results with the image badge", async () => {
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({
        id: "image-asset:assets/logos/brand.png",
        title: "brand.png",
        subtitle: "assets/logos/brand.png",
        kind: "image",
        close_launcher_on_success: true,
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "brand" } });

    expect(await screen.findByText("brand.png")).toBeTruthy();
    expect(await screen.findByText("Image")).toBeTruthy();
  });

  it("renders leading-space window results with the window badge", async () => {
    vi.mocked(launcherApi.searchBrowserTabs).mockResolvedValue([
      searchResult({
        id: "open-window:4242:10:20:1440:900",
        title: "Project Board",
        subtitle: "Linear",
        kind: "open_window",
        close_launcher_on_success: true,
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: " project" } });

    expect(await screen.findByText("Project Board")).toBeTruthy();
    expect(await screen.findByText("Window")).toBeTruthy();
  });

  it("shows all tabs when the query is only a leading space", async () => {
    vi.mocked(launcherApi.searchBrowserTabs).mockResolvedValue([
      searchResult({
        id: "browser-tab:chrome:window-1:1",
        title: "Rayon",
        subtitle: "github.com",
        kind: "browser_tab",
        close_launcher_on_success: true,
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: " " } });

    expect(await screen.findByText("Rayon")).toBeTruthy();
    expect(launcherApi.searchBrowserTabs).toHaveBeenCalledWith("", true);
    expect(input.getAttribute("placeholder")).toBe("Search open windows and tabs");
  });

  it("refreshes browser tabs only when entering tab mode", async () => {
    vi.mocked(launcherApi.searchBrowserTabs).mockResolvedValue([
      searchResult({
        id: "browser-tab:chrome:window-1:1",
        title: "Issue 15",
        subtitle: "github.com",
        kind: "browser_tab",
        close_launcher_on_success: true,
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: " issue" } });
    expect(await screen.findByText("Issue 15")).toBeTruthy();

    fireEvent.change(input, { target: { value: " issue 15" } });

    await waitFor(() => {
      expect(launcherApi.searchBrowserTabs).toHaveBeenNthCalledWith(1, "issue", true);
      expect(launcherApi.searchBrowserTabs).toHaveBeenNthCalledWith(2, "issue 15", false);
    });
  });

  it("leaves browser tab mode when the leading space is removed", async () => {
    vi.mocked(launcherApi.searchBrowserTabs).mockResolvedValue([
      searchResult({
        id: "browser-tab:chrome:window-1:1",
        title: "Issue 15",
        subtitle: "github.com",
        kind: "browser_tab",
        close_launcher_on_success: true,
      }),
    ]);
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({ id: "echo", title: "Echo" }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: " issue" } });
    expect(await screen.findByText("Issue 15")).toBeTruthy();

    fireEvent.change(input, { target: { value: "issue" } });

    expect(await screen.findByText("Echo")).toBeTruthy();
    expect(launcherApi.searchLauncher).toHaveBeenCalledWith("issue");
    expect(input.getAttribute("placeholder")).toBe("Type a command");
    expect(input.getAttribute("data-mode")).toBe("default");
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
      expect(launcherApi.executeLauncherCommand).toHaveBeenCalledWith(
        "apps.reindex",
        {
          enabled: {
            type: "boolean",
            value: true,
          },
        },
        [],
      );
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

  it("restores focus after selecting a clipboard history item", async () => {
    const session = interactiveSession({
      command_id: "clipboard",
      title: "Clipboard History",
      subtitle: "Search your recent clipboard entries",
      input_placeholder: "Search clipboard history",
      completion_behavior: "hide_launcher_and_restore_focus",
      results: [
        {
          id: "1",
          title: "deploy preview url",
          subtitle: "https://example.com/preview",
        },
      ],
    });

    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({
        id: "clipboard",
        title: "Clipboard History",
        subtitle: "Browse and recopy your recent clipboard items",
        starts_interactive_session: true,
      }),
    ]);
    vi.mocked(launcherApi.executeLauncherCommand).mockResolvedValue({
      kind: "started_session",
      session,
    });
    vi.mocked(launcherApi.searchInteractiveSession).mockResolvedValue(session);
    vi.mocked(launcherApi.submitLauncherInteractiveSelection).mockResolvedValue({
      kind: "completed",
      output: "copied clipboard item",
      completion_behavior: "hide_launcher_and_restore_focus",
    });

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "clipboard" } });

    expect(await screen.findByText("Clipboard History")).toBeTruthy();
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByText("deploy preview url")).toBeTruthy();

    await userEvent.keyboard("{Enter}");

    await waitFor(() => {
      expect(launcherApi.submitLauncherInteractiveSelection).toHaveBeenCalledWith(
        "session-1",
        "",
        "1",
      );
    });
    await waitFor(() => {
      expect(launcherApi.hideLauncherAndRestoreFocus).toHaveBeenCalled();
    });
    expect(launcherApi.hideLauncher).not.toHaveBeenCalled();
  });

  it("executes raw argv commands directly from one line", async () => {
    vi.mocked(launcherApi.searchLauncher)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([
        searchResult({
          id: "user.git-status",
          title: "Git Status",
          subtitle: "Run git status",
          input_mode: "raw_argv",
          keywords: ["git-status"],
        }),
      ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: 'git-status "/tmp/my repo"' } });

    expect(await screen.findByText("Git Status")).toBeTruthy();
    expect(screen.queryByText("Step 1 of 1")).toBeNull();

    await userEvent.keyboard("{Enter}");

    await waitFor(() => {
      expect(launcherApi.executeLauncherCommand).toHaveBeenCalledWith("user.git-status", {}, [
        "/tmp/my repo",
      ]);
    });
  });

  it("executes raw argv commands from an exact keyword alias with trailing args", async () => {
    vi.mocked(launcherApi.searchLauncher)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([
        searchResult({
          id: "user.teleport",
          title: "Teleport",
          subtitle: "Run teleport",
          input_mode: "raw_argv",
          keywords: ["tp"],
        }),
      ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "tp 1" } });

    expect(await screen.findByText("Teleport")).toBeTruthy();

    await userEvent.keyboard("{Enter}");

    await waitFor(() => {
      expect(launcherApi.executeLauncherCommand).toHaveBeenCalledWith("user.teleport", {}, ["1"]);
    });
  });

  it("executes raw argv commands with trailing args when the full query already returns the alias match", async () => {
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({
        id: "user.teleport",
        title: "Teleport",
        subtitle: "Run teleport",
        input_mode: "raw_argv",
        keywords: ["tp"],
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "tp 1" } });

    expect(await screen.findByText("Teleport")).toBeTruthy();

    await userEvent.keyboard("{Enter}");

    await waitFor(() => {
      expect(launcherApi.executeLauncherCommand).toHaveBeenCalledWith("user.teleport", {}, ["1"]);
    });
  });

  it("executes raw argv commands from an exact title alias with trailing args", async () => {
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({
        id: "user.talkpad-copy",
        title: "tp",
        subtitle: "Copy a Talkpad note to the clipboard",
        input_mode: "raw_argv",
        keywords: ["talkpad", "note", "clipboard", "copy", "offset"],
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "tp 1" } });

    expect(await screen.findByText("tp")).toBeTruthy();

    await userEvent.keyboard("{Enter}");

    await waitFor(() => {
      expect(launcherApi.executeLauncherCommand).toHaveBeenCalledWith("user.talkpad-copy", {}, [
        "1",
      ]);
    });
  });

  it("blocks raw argv execution when the input has an unclosed quote", async () => {
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({
        id: "user.echo",
        title: "Echo",
        subtitle: "Run echo",
        input_mode: "raw_argv",
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: 'echo "hello' } });

    expect(await screen.findByText("Echo")).toBeTruthy();
    await userEvent.keyboard("{Enter}");

    expect(await screen.findByText("Command input contains an unclosed quote.")).toBeTruthy();
    expect(launcherApi.executeLauncherCommand).not.toHaveBeenCalledWith("user.echo", {}, []);
  });

  it("hides the launcher and restores focus for successful auto-close commands", async () => {
    vi.mocked(launcherApi.searchLauncher).mockResolvedValue([
      searchResult({
        id: "user.git-status",
        title: "Git Status",
        close_launcher_on_success: true,
      }),
    ]);

    render(<App />);

    const input = screen.getByLabelText("Command search");
    fireEvent.change(input, { target: { value: "git" } });

    expect(await screen.findByText("Git Status")).toBeTruthy();
    await userEvent.keyboard("{Enter}");

    await waitFor(() => {
      expect(launcherApi.hideLauncherAndRestoreFocus).toHaveBeenCalled();
    });
    expect(screen.queryByText("done")).toBeNull();
  });
});
