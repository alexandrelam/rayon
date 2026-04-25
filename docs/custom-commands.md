# Custom Commands

`rayon` can load custom commands from TOML files in your config directory. This lets you add commands without changing the app code.

## Config Location

`rayon` looks for `.toml` files in one of these directories:

- `$XDG_CONFIG_HOME/rayon` when `XDG_CONFIG_HOME` is set
- `~/.config/rayon` otherwise

Every `.toml` file in that directory is loaded, in sorted filename order.

## Manifest Format

Each manifest file must define a top-level `plugin_id` and one or more `[[commands]]` entries.

```toml
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"
base_args = ["hello from rayon"]
```

Supported command fields:

- `id`: unique command id
- `title`: label shown in the launcher
- `program`: executable to run
- `subtitle`: optional secondary text
- `keywords`: optional search keywords
- `base_args`: optional fixed arguments always passed before user input
- `working_dir`: optional working directory
- `env`: optional environment variables
- `arguments`: optional interactive arguments collected by the UI before execution

## Arguments

Custom command arguments support two types:

- `string`
- `boolean`

Each argument entry supports these fields:

- `id`: unique argument id within the command
- `label`: prompt shown in the UI
- `type`: `string` or `boolean`
- `required`: optional, defaults to `false`
- `flag`: optional CLI flag such as `--message`
- `positional`: optional positional index
- `default_string`: optional default for `string`
- `default_boolean`: optional default for `boolean`

Example with both string and boolean arguments:

```toml
plugin_id = "user.commands"

[[commands]]
id = "user.git-status"
title = "Git Status"
subtitle = "Run git status in a repo"
program = "/usr/bin/git"
base_args = ["status"]
working_dir = "../work/project"
keywords = ["git", "repo", "status"]

[commands.env]
FORCE_COLOR = "1"

[[commands.arguments]]
id = "path"
label = "Repository Path"
type = "string"
required = true
positional = 0

[[commands.arguments]]
id = "short"
label = "Short Output"
type = "boolean"
flag = "--short"
default_boolean = true
```

## Execution Rules

- If `program` or `working_dir` is a relative path, it is resolved relative to the manifest file's directory.
- `base_args` are added first.
- String arguments with a `flag` become `flag value`.
- Boolean arguments with a `flag` only add the flag when the value is `true`.
- Positional arguments are appended in `positional` order.
- If a command writes to `stdout`, that text is shown in the app.
- If `stdout` is empty and `stderr` has content, `stderr` is shown instead.
- If both are empty, the app shows a generic completion message.
- If the process exits with a non-zero status, the output is surfaced as an execution error.

## Instructions For Codex Or Claude Code

When you want an assistant to add a new custom command, give it these inputs:

1. The command title you want shown in `rayon`
2. A stable command id such as `user.open-project`
3. The executable path for `program`
4. Any fixed `base_args`
5. An optional `working_dir`
6. Any optional environment variables for `[commands.env]`
7. Any interactive arguments:
   - label shown to the user
   - `type`: `string` or `boolean`
   - whether it is required
   - whether it should be passed as a `flag` or a `positional` argument
   - any default value

Use this prompt shape with Codex or Claude Code:

```text
Create a new rayon custom command in ~/.config/rayon/commands.toml.

Command title: Open Project
Command id: user.open-project
Program: /usr/bin/open
Base args: []
Working directory: none
Environment variables: none

Arguments:
- path: string, required, positional 0, label "Project Path"
```

Ask the assistant to write valid TOML using only the supported keys listed above.

## Troubleshooting

- Nothing appears in `rayon`: confirm the file is under `$XDG_CONFIG_HOME/rayon` or `~/.config/rayon`.
- TOML parse error: check for missing quotes, invalid tables, or unsupported keys.
- Command fails immediately: verify that `program` points to a real executable.
- Relative path does not resolve: remember relative paths are resolved from the manifest file's directory, not your shell's current directory.
- Boolean argument rejected: enter a true/false style value such as `true`, `false`, `yes`, `no`, `1`, `0`, `on`, or `off`.
- Config change not visible yet: restart `rayon` so it reloads commands from the config directory.
