import { describe, expect, it } from "vitest";
import type { PendingExecution, SearchResult } from "./commandExecution";
import { getLauncherViewState, isLauncherIdle, shouldRunSearch } from "./launcherViewState";

const searchResult = (overrides: Partial<SearchResult> = {}): SearchResult => ({
  id: "hello",
  title: "Hello",
  subtitle: "Built-in greeting",
  icon_path: null,
  kind: "command",
  owner_plugin_id: "builtin.hello",
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

describe("launcher view state", () => {
  it("keeps the header visible while idle", () => {
    expect(
      isLauncherIdle({
        query: "",
        executionResult: "",
        error: "",
        pendingExecution: null,
      }),
    ).toBe(true);

    expect(
      getLauncherViewState({
        query: "",
        results: [],
        executionResult: "",
        error: "",
        pendingExecution: null,
      }),
    ).toEqual({
      idle: true,
      showHeader: true,
      showResults: false,
      showEmptyResults: false,
      showFooter: false,
      showingArgumentPrompt: false,
    });
  });

  it("runs search only for non-empty queries outside argument entry", () => {
    expect(shouldRunSearch({ query: "", pendingExecution: null })).toBe(false);
    expect(shouldRunSearch({ query: "arc", pendingExecution: null })).toBe(true);
    expect(shouldRunSearch({ query: "arc", pendingExecution })).toBe(false);
  });

  it("shows the empty state after a typed query returns no matches", () => {
    expect(
      getLauncherViewState({
        query: "missing",
        results: [],
        executionResult: "",
        error: "",
        pendingExecution: null,
      }),
    ).toMatchObject({
      idle: false,
      showHeader: true,
      showResults: true,
      showEmptyResults: true,
      showFooter: false,
      showingArgumentPrompt: false,
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
      }),
    ).toEqual({
      idle: false,
      showHeader: true,
      showResults: false,
      showEmptyResults: false,
      showFooter: true,
      showingArgumentPrompt: true,
    });
  });
});
