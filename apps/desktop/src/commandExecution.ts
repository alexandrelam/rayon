export type CommandArgumentType = "string" | "boolean";

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
  kind: "command" | "application" | "bookmark";
  owner_plugin_id: string | null;
  arguments: CommandArgumentDefinition[];
};

export type CommandExecutionResult = {
  output: string;
};

export type CommandArgumentValue =
  | { type: "string"; value: string }
  | { type: "boolean"; value: boolean };

export type PendingExecution = {
  commandId: string;
  commandTitle: string;
  arguments: CommandArgumentDefinition[];
  values: Partial<Record<string, CommandArgumentValue>>;
  currentIndex: number;
};

export type PendingExecutionStep =
  | { kind: "error"; message: string }
  | { kind: "advance"; pendingExecution: PendingExecution }
  | { kind: "execute"; commandId: string; argumentsMap: Record<string, CommandArgumentValue> };

export function beginPendingExecution(result: SearchResult): PendingExecution | null {
  if (result.kind !== "command" || result.arguments.length === 0) {
    return null;
  }

  return {
    commandId: result.id,
    commandTitle: result.title,
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
