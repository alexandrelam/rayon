import { describe, expect, it } from "vitest";
import type { InteractiveSessionState, PendingExecution, SearchResult } from "./commandExecution";
import { getLauncherViewState, isLauncherIdle, shouldRunSearch } from "./launcherViewState";

const searchResult = (overrides: Partial<SearchResult> = {}): SearchResult => ({
  id: "echo",
  title: "Echo",
  subtitle: "Run echo",
  icon_path: null,
  kind: "command",
  owner_plugin_id: "user.commands",
  keywords: [],
  starts_interactive_session: false,
  input_mode: "structured",
  arguments: [],
  ...overrides,
});

const pendingExecution: PendingExecution = {
  commandId: "echo",
  commandTitle: "Echo",
  arguments: [
    {
      id: "message",
      label: "Message",
      argument_type: "string",
      required: true,
      flag: null,
      positional: 0,
      default_value: null,
    },
  ],
  values: {},
  currentIndex: 0,
};

const interactiveSession: InteractiveSessionState = {
  session_id: "session-1",
  command_id: "kill",
  title: "Kill Process",
  subtitle: "Search by app, process, or port",
  input_placeholder: "Search process name or port 8080",
  query: "",
  is_loading: false,
  results: [],
  message: null,
};

describe("launcher view state", () => {
  it("keeps the header visible while idle", () => {
    expect(
      isLauncherIdle({
        query: "",
        executionResult: "",
        error: "",
        pendingExecution: null,
        interactiveSession: null,
      }),
    ).toBe(true);

    expect(
      getLauncherViewState({
        query: "",
        results: [],
        executionResult: "",
        error: "",
        pendingExecution: null,
        interactiveSession: null,
      }),
    ).toEqual({
      idle: true,
      showHeader: true,
      showResults: false,
      showEmptyResults: false,
      showFooter: false,
      showingArgumentPrompt: false,
      showingInteractiveSession: false,
    });
  });

  it("runs search only for non-empty queries outside argument entry", () => {
    expect(shouldRunSearch({ query: "", pendingExecution: null, interactiveSession: null })).toBe(
      false,
    );
    expect(
      shouldRunSearch({ query: "arc", pendingExecution: null, interactiveSession: null }),
    ).toBe(true);
    expect(shouldRunSearch({ query: "arc", pendingExecution, interactiveSession: null })).toBe(
      false,
    );
    expect(shouldRunSearch({ query: "arc", pendingExecution: null, interactiveSession })).toBe(
      false,
    );
  });

  it("shows the empty state after a typed query returns no matches", () => {
    expect(
      getLauncherViewState({
        query: "missing",
        results: [],
        executionResult: "",
        error: "",
        pendingExecution: null,
        interactiveSession: null,
      }),
    ).toMatchObject({
      idle: false,
      showHeader: true,
      showResults: true,
      showEmptyResults: true,
      showFooter: false,
      showingArgumentPrompt: false,
      showingInteractiveSession: false,
    });
  });

  it("returns to idle after clearing the query", () => {
    expect(
      getLauncherViewState({
        query: "",
        results: [],
        executionResult: "",
        error: "",
        pendingExecution: null,
        interactiveSession: null,
      }).idle,
    ).toBe(true);
  });

  it("keeps argument entry visible with an empty query", () => {
    expect(
      getLauncherViewState({
        query: "",
        results: [searchResult()],
        executionResult: "",
        error: "",
        pendingExecution,
        interactiveSession: null,
      }),
    ).toEqual({
      idle: false,
      showHeader: true,
      showResults: false,
      showEmptyResults: false,
      showFooter: true,
      showingArgumentPrompt: true,
      showingInteractiveSession: false,
    });
  });

  it("keeps interactive sessions visible with an empty query", () => {
    expect(
      getLauncherViewState({
        query: "",
        results: [],
        executionResult: "",
        error: "",
        pendingExecution: null,
        interactiveSession,
      }),
    ).toEqual({
      idle: false,
      showHeader: true,
      showResults: true,
      showEmptyResults: false,
      showFooter: true,
      showingArgumentPrompt: false,
      showingInteractiveSession: true,
    });
  });
});
