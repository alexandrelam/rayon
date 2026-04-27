import { describe, expect, it } from "vitest";
import {
  beginPendingExecution,
  currentArgumentInputValue,
  matchesExactKeywordAlias,
  parseCommandLine,
  type PendingExecution,
  resolveInlineArgv,
  resolvePendingExecutionStep,
  scheduleAfterNextPaint,
  type SearchResult,
} from "./commandExecution";

const toggleArgument = {
  id: "enabled",
  label: "Enabled",
  argument_type: "boolean" as const,
  required: true,
  flag: "--enabled",
  positional: null,
  default_value: null,
};

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

describe("commandExecution helpers", () => {
  it("only starts pending execution for structured commands with arguments", () => {
    expect(beginPendingExecution(searchResult())).toBeNull();

    expect(
      beginPendingExecution(
        searchResult({
          input_mode: "raw_argv",
          arguments: [toggleArgument],
        }),
      ),
    ).toBeNull();

    expect(
      beginPendingExecution(
        searchResult({
          arguments: [toggleArgument],
        }),
      ),
    ).toEqual({
      commandId: "echo",
      commandTitle: "Echo",
      arguments: [toggleArgument],
      values: {},
      currentIndex: 0,
    });

    expect(
      beginPendingExecution(
        searchResult({
          kind: "application",
          arguments: [toggleArgument],
        }),
      ),
    ).toBeNull();

    expect(
      beginPendingExecution(
        searchResult({
          kind: "bookmark",
          arguments: [toggleArgument],
        }),
      ),
    ).toBeNull();
  });

  it("uses default values when rendering the active argument input", () => {
    const pendingExecution: PendingExecution = {
      commandId: "echo",
      commandTitle: "Echo",
      arguments: [
        {
          id: "message",
          label: "Message",
          argument_type: "string",
          required: false,
          flag: null,
          positional: 0,
          default_value: { type: "string", value: "hello" },
        },
      ],
      values: {},
      currentIndex: 0,
    };

    expect(currentArgumentInputValue(pendingExecution, "")).toBe("hello");
  });

  it("returns a validation error for invalid boolean input", () => {
    const pendingExecution = beginPendingExecution(
      searchResult({
        arguments: [toggleArgument],
      }),
    );
    if (!pendingExecution) {
      throw new Error("pending execution should be created for commands with arguments");
    }

    expect(resolvePendingExecutionStep(pendingExecution, "maybe")).toEqual({
      kind: "error",
      message: "Enabled expects true/false",
    });
  });

  it("returns an execute step with the parsed arguments", () => {
    const pendingExecution = beginPendingExecution(
      searchResult({
        id: "apps.reindex",
        title: "Reindex Search",
        arguments: [toggleArgument],
      }),
    );
    if (!pendingExecution) {
      throw new Error("pending execution should be created for commands with arguments");
    }

    expect(resolvePendingExecutionStep(pendingExecution, "true")).toEqual({
      kind: "execute",
      commandId: "apps.reindex",
      argumentsMap: {
        enabled: {
          type: "boolean",
          value: true,
        },
      },
    });
  });

  it("parses shell-style command lines", () => {
    expect(parseCommandLine('echo "hello world" path\\ with\\ spaces')).toEqual({
      kind: "success",
      tokens: ["echo", "hello world", "path with spaces"],
    });
  });

  it("returns a parsing error for an unclosed quote", () => {
    expect(parseCommandLine('echo "hello')).toEqual({
      kind: "error",
      message: "Command input contains an unclosed quote.",
    });
  });

  it("uses fallback argv for raw custom commands", () => {
    expect(
      resolveInlineArgv(
        searchResult({
          input_mode: "raw_argv",
        }),
        'git status "/tmp/my repo"',
        ["/tmp/my repo"],
      ),
    ).toEqual({
      kind: "fallback",
      argv: ["/tmp/my repo"],
    });
  });

  it("extracts argv from an exact keyword alias", () => {
    expect(
      resolveInlineArgv(
        searchResult({
          input_mode: "raw_argv",
          keywords: ["tp"],
        }),
        'tp "/tmp/my repo"',
      ),
    ).toEqual({
      kind: "matched",
      argv: ["/tmp/my repo"],
    });
  });

  it("extracts argv from an exact title alias", () => {
    expect(
      resolveInlineArgv(
        searchResult({
          title: "tp",
          input_mode: "raw_argv",
          keywords: ["talkpad"],
        }),
        'tp "/tmp/my repo"',
      ),
    ).toEqual({
      kind: "matched",
      argv: ["/tmp/my repo"],
    });
  });

  it("matches exact keyword aliases case-insensitively", () => {
    expect(
      matchesExactKeywordAlias(
        searchResult({
          input_mode: "raw_argv",
          keywords: ["tp"],
        }),
        ["TP"],
      ),
    ).toBe(true);
  });

  it("matches exact title aliases case-insensitively", () => {
    expect(
      matchesExactKeywordAlias(
        searchResult({
          title: "tp",
          input_mode: "raw_argv",
          keywords: ["talkpad"],
        }),
        ["TP"],
      ),
    ).toBe(true);
  });

  it("schedules work after the next paint boundary", () => {
    const events: string[] = [];
    const queuedFrames: (() => void)[] = [];
    const queuedTasks: (() => void)[] = [];

    scheduleAfterNextPaint(
      () => {
        events.push("callback");
      },
      (callback) => {
        events.push("frame-scheduled");
        queuedFrames.push(callback);
      },
      (callback) => {
        events.push("task-scheduled");
        queuedTasks.push(callback);
      },
    );

    expect(events).toEqual(["frame-scheduled"]);
    expect(queuedFrames).toHaveLength(1);

    queuedFrames[0]?.();
    expect(events).toEqual(["frame-scheduled", "task-scheduled"]);
    expect(queuedTasks).toHaveLength(1);

    queuedTasks[0]?.();
    expect(events).toEqual(["frame-scheduled", "task-scheduled", "callback"]);
  });
});
