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
base_args = ["status"]
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

## Direct Execution

Custom commands always run in one line. Type the command alias in the launcher, add any trailing arguments, then press `Enter`.

Example:

```toml
plugin_id = "user.commands"

[[commands]]
id = "user.git-status"
title = "Git Status"
program = "/usr/bin/git"
base_args = ["status"]
keywords = ["git-status"]
```

With that config, typing `git-status ~/work/repo` in the launcher executes:

```text
/usr/bin/git status ~/work/repo
```

Trailing arguments use basic shell-style tokenization:

- unquoted whitespace splits arguments
- single quotes preserve spaces
- double quotes preserve spaces
- backslashes escape the next character

An exact command `title` also works as a runnable alias. Use `keywords` for extra aliases you want to type directly. Prefer single-token aliases so trailing arguments stay unambiguous.

## Execution Rules

- If `program` or `working_dir` is a relative path, it is resolved relative to the manifest file's directory.
- `base_args` are added first.
- Launcher input after the matched command title or alias is appended directly as argv tokens.
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
7. Optional `keywords` you want to type as aliases in the launcher

Use this prompt shape with Codex or Claude Code:

```text
Create a new rayon custom command in ~/.config/rayon/commands.toml.

Command title: Open Project
Command id: user.open-project
Program: /usr/bin/open
Base args: []
Working directory: none
Environment variables: none
Keywords: ["project"]
```

Ask the assistant to write valid TOML using only the supported keys listed above.

## Troubleshooting

- Nothing appears in `rayon`: confirm the file is under `$XDG_CONFIG_HOME/rayon` or `~/.config/rayon`.
- TOML parse error: check for missing quotes, invalid tables, or unsupported keys.
- Command fails immediately: verify that `program` points to a real executable.
- Relative path does not resolve: remember relative paths are resolved from the manifest file's directory, not your shell's current directory.
- Launcher input rejected: check for an unclosed quote or trailing backslash in the launcher input.
- Config change not visible yet: run `apps.reindex` in `rayon` to reload commands from the config directory. If reload fails, `rayon` keeps the previous live state and shows the config error.
