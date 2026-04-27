export type CommandArgumentType = "string" | "boolean";
export type CommandInputMode = "structured" | "raw_argv";

export type CommandArgumentDefinition = {
  id: string;
  label: string;
  argument_type: CommandArgumentType;
  required: boolean;
  flag: string | null;
  positional: number | null;
  default_value: { type: "string"; value: string } | { type: "boolean"; value: boolean } | null;
};

export type SearchResult = {
  id: string;
  title: string;
  subtitle: string | null;
  icon_path: string | null;
  kind: "command" | "application" | "bookmark" | "browser_tab";
  owner_plugin_id: string | null;
  keywords: string[];
  starts_interactive_session: boolean;
  close_launcher_on_success: boolean;
  input_mode: CommandInputMode;
  arguments: CommandArgumentDefinition[];
};

export type CommandExecutionResult = {
  output: string;
};

export type InteractiveSessionResult = {
  id: string;
  title: string;
  subtitle: string | null;
};

export type InteractiveSessionState = {
  session_id: string;
  command_id: string;
  title: string;
  subtitle: string | null;
  input_placeholder: string;
  completion_behavior: "hide_launcher" | "hide_launcher_and_restore_focus";
  query: string;
  is_loading: boolean;
  results: InteractiveSessionResult[];
  message: string | null;
};

export type CommandInvocationResult =
  | { kind: "completed"; output: string }
  | { kind: "started_session"; session: InteractiveSessionState };

export type InteractiveSessionSubmitResult =
  | { kind: "updated_session"; session: InteractiveSessionState }
  | {
      kind: "completed";
      output: string;
      completion_behavior: "hide_launcher" | "hide_launcher_and_restore_focus";
    };

export type CommandArgumentValue =
  | { type: "string"; value: string }
  | { type: "boolean"; value: boolean };

export type PendingExecution = {
  commandId: string;
  commandTitle: string;
  closeLauncherOnSuccess: boolean;
  arguments: CommandArgumentDefinition[];
  values: Partial<Record<string, CommandArgumentValue>>;
  currentIndex: number;
};

export type PendingExecutionStep =
  | { kind: "error"; message: string }
  | { kind: "advance"; pendingExecution: PendingExecution }
  | { kind: "execute"; commandId: string; argumentsMap: Record<string, CommandArgumentValue> };

export type ParsedCommandLine =
  | { kind: "success"; tokens: string[] }
  | { kind: "error"; message: string };

export type InlineArgvResolution =
  | { kind: "matched"; argv: string[] }
  | { kind: "fallback"; argv: string[] }
  | { kind: "error"; message: string };

type FrameScheduler = (callback: () => void) => void;
type TaskScheduler = (callback: () => void) => void;

export function beginPendingExecution(result: SearchResult): PendingExecution | null {
  if (
    result.kind !== "command" ||
    result.input_mode !== "structured" ||
    result.arguments.length === 0
  ) {
    return null;
  }

  return {
    commandId: result.id,
    commandTitle: result.title,
    closeLauncherOnSuccess: result.close_launcher_on_success,
    arguments: result.arguments,
    values: {},
    currentIndex: 0,
  };
}

export function currentArgument(
  pendingExecution: PendingExecution | null,
): CommandArgumentDefinition | null {
  if (!pendingExecution) {
    return null;
  }

  return pendingExecution.arguments[pendingExecution.currentIndex] ?? null;
}

export function currentArgumentInputValue(
  pendingExecution: PendingExecution | null,
  query: string,
): string {
  const argument = currentArgument(pendingExecution);
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

export function parseArgumentValue(
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

export function resolvePendingExecutionStep(
  pendingExecution: PendingExecution,
  query: string,
): PendingExecutionStep {
  const argument = currentArgument(pendingExecution);
  if (!argument) {
    return {
      kind: "execute",
      commandId: pendingExecution.commandId,
      argumentsMap: toArgumentsMap(pendingExecution.values),
    };
  }

  const parsedValue = parseArgumentValue(argument, query);
  if (typeof parsedValue === "string") {
    return { kind: "error", message: parsedValue };
  }

  const nextValues = { ...pendingExecution.values };
  if (parsedValue) {
    nextValues[argument.id] = parsedValue;
  } else {
    const { [argument.id]: _deletedValue, ...remainingValues } = nextValues;
    return buildPendingExecutionStep(pendingExecution, remainingValues);
  }

  return buildPendingExecutionStep(pendingExecution, nextValues);
}

export function scheduleAfterNextPaint(
  callback: () => void,
  scheduleFrame: FrameScheduler = (nextCallback) => {
    requestAnimationFrame(() => {
      nextCallback();
    });
  },
  scheduleTask: TaskScheduler = (nextCallback) => {
    setTimeout(() => {
      nextCallback();
    }, 0);
  },
) {
  scheduleFrame(() => {
    scheduleTask(callback);
  });
}

export function parseCommandLine(query: string): ParsedCommandLine {
  const tokens: string[] = [];
  let current = "";
  let quote: "'" | '"' | null = null;
  let escaping = false;

  for (const character of query) {
    if (escaping) {
      current += character;
      escaping = false;
      continue;
    }

    if (quote === "'") {
      if (character === "'") {
        quote = null;
      } else {
        current += character;
      }
      continue;
    }

    if (quote === '"') {
      if (character === '"') {
        quote = null;
        continue;
      }
      if (character === "\\") {
        escaping = true;
        continue;
      }
      current += character;
      continue;
    }

    if (character === "\\") {
      escaping = true;
      continue;
    }

    if (character === "'" || character === '"') {
      quote = character;
      continue;
    }

    if (/\s/.test(character)) {
      if (current !== "") {
        tokens.push(current);
        current = "";
      }
      continue;
    }

    current += character;
  }

  if (escaping) {
    return {
      kind: "error",
      message: "Command input ends with an unfinished escape sequence.",
    };
  }

  if (quote) {
    return {
      kind: "error",
      message: "Command input contains an unclosed quote.",
    };
  }

  if (current !== "") {
    tokens.push(current);
  }

  return { kind: "success", tokens };
}

export function resolveInlineArgv(
  result: SearchResult,
  query: string,
  fallbackArgv: string[] = [],
): InlineArgvResolution {
  if (result.kind !== "command" || result.input_mode !== "raw_argv") {
    return { kind: "fallback", argv: [] };
  }

  const parsed = parseCommandLine(query);
  if (parsed.kind === "error") {
    return { kind: "error", message: parsed.message };
  }

  const matchedArgv = argvFromExactKeywordAlias(result, parsed.tokens);
  if (matchedArgv) {
    return { kind: "matched", argv: matchedArgv };
  }

  return { kind: "fallback", argv: fallbackArgv };
}

export function matchesExactKeywordAlias(result: SearchResult, tokens: string[]): boolean {
  return argvFromExactKeywordAlias(result, tokens) !== null;
}

function buildPendingExecutionStep(
  pendingExecution: PendingExecution,
  values: Partial<Record<string, CommandArgumentValue>>,
): PendingExecutionStep {
  const nextIndex = pendingExecution.currentIndex + 1;
  if (nextIndex >= pendingExecution.arguments.length) {
    return {
      kind: "execute",
      commandId: pendingExecution.commandId,
      argumentsMap: toArgumentsMap(values),
    };
  }

  return {
    kind: "advance",
    pendingExecution: {
      ...pendingExecution,
      values,
      currentIndex: nextIndex,
    },
  };
}

function toArgumentsMap(
  values: Partial<Record<string, CommandArgumentValue>>,
): Record<string, CommandArgumentValue> {
  return Object.fromEntries(
    Object.entries(values).filter((entry): entry is [string, CommandArgumentValue] => {
      return entry[1] !== undefined;
    }),
  );
}

function argvFromExactKeywordAlias(result: SearchResult, tokens: string[]): string[] | null {
  if (result.kind !== "command" || result.input_mode !== "raw_argv") {
    return null;
  }

  const normalizedTokens = tokens.map(normalizeToken).filter((token) => token !== "");
  const aliases = [result.title, ...result.keywords];
  for (const alias of aliases) {
    const aliasTokens = tokenizeAlias(alias);
    if (aliasTokens.length === 0 || aliasTokens.length > normalizedTokens.length) {
      continue;
    }

    const isMatch = aliasTokens.every((token, index) => token === normalizedTokens[index]);
    if (isMatch) {
      return tokens.slice(aliasTokens.length);
    }
  }

  return null;
}

function tokenizeAlias(value: string): string[] {
  return value
    .split(/\s+/)
    .map(normalizeToken)
    .filter((token) => token !== "");
}

function normalizeToken(value: string): string {
  return value.trim().toLowerCase();
}
