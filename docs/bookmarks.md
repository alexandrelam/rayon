# Bookmarks

`rayon` can load bookmarks from TOML files in your config directory. This lets you add searchable links without changing the app code.

## Config Location

`rayon` looks for `.toml` files in one of these directories:

- `$XDG_CONFIG_HOME/rayon` when `XDG_CONFIG_HOME` is set
- `~/.config/rayon` otherwise

Every `.toml` file in that directory is loaded, in sorted filename order.

## Manifest Format

Each manifest file must define a top-level `plugin_id` and one or more `[[bookmarks]]` entries.

```toml
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.github"
title = "GitHub"
url = "https://github.com"
subtitle = "Code hosting"
keywords = ["git", "repos", "source"]
```

Supported bookmark fields:

- `id`: unique bookmark id
- `title`: label shown in the launcher
- `url`: absolute URL opened when selected
- `subtitle`: optional secondary text
- `keywords`: optional search keywords

## Opening Rules

- `url` must be an absolute URL with a scheme such as `https://`.
- When you press `Enter` on a bookmark result, `rayon` opens the URL through macOS using the system default browser.
- Bookmark ids must be unique across bookmarks.
- Bookmark ids must not conflict with command ids.

## Search Behavior

- `rayon` indexes the bookmark id, title, subtitle, URL, plugin id, and keywords.
- Bookmarks appear in the launcher search results just like apps and commands.
- Bookmarks do not prompt for arguments. Pressing `Enter` opens them immediately.

## Instructions For Codex Or Claude Code

When you want an assistant to add a new bookmark, give it these inputs:

1. The bookmark title you want shown in `rayon`
2. A stable bookmark id such as `user.github`
3. The absolute URL to open
4. An optional subtitle
5. Any optional search keywords
6. The target manifest path under `$XDG_CONFIG_HOME/rayon` or `~/.config/rayon`

Use this prompt shape with Codex or Claude Code:

```text
Create a new rayon bookmark in ~/.config/rayon/bookmarks.toml.

Plugin id: user.bookmarks
Bookmark title: GitHub
Bookmark id: user.github
URL: https://github.com
Subtitle: Code hosting
Keywords: ["git", "repos", "source"]
```

Ask the assistant to write valid TOML using only the supported keys listed above.

## Example Manifests

Single bookmark:

```toml
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.calendar"
title = "Google Calendar"
url = "https://calendar.google.com"
subtitle = "Schedule"
keywords = ["calendar", "meetings", "events"]
```

Multiple bookmarks in one file:

```toml
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.github"
title = "GitHub"
url = "https://github.com"
keywords = ["git", "repos"]

[[bookmarks]]
id = "user.linear"
title = "Linear"
url = "https://linear.app"
subtitle = "Issue tracker"
keywords = ["issues", "tickets", "projects"]
```

## Troubleshooting

- Nothing appears in `rayon`: confirm the file is under `$XDG_CONFIG_HOME/rayon` or `~/.config/rayon`.
- TOML parse error: check for missing quotes, invalid tables, or unsupported keys.
- Bookmark does not open: verify that `url` is absolute and includes `http://` or `https://`.
- Config change not visible yet: restart `rayon` so it reloads bookmarks from the config directory.
- Bookmark conflicts with another item: choose a different `id` that does not overlap with existing bookmarks or commands.
