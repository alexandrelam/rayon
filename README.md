# rayon

Keyboard-first desktop launcher built with Rust, Tauri, and React.

## Goal

`rayon` is moving toward a plugin-oriented launcher:

- built-in modules and contributor plugins should use the same command model
- searchable items should be indexed through Tantivy
- the frontend should stay a thin shell over a generic backend API

Current code is an early foundation: built-in providers are wired in statically, and Tantivy is currently used for app indexing only.

## Layout

- `apps/desktop`: Tauri shell and command palette UI
- `crates/types`: shared models
- `crates/core`: command registry and orchestration
- `crates/features`: built-in providers
- `crates/platform`: OS integration
- `crates/db`: Tantivy-backed indexing

## Commands
- `pnpm dev`: run the frontend dev server
- `pnpm tauri dev`: run the desktop app
- `cargo test`: run Rust tests across the workspace

## Custom Commands

`rayon` can load user-defined commands from your config directory. See [Custom Commands](/Users/alex/Documents/rayon/docs/custom-commands.md) for the manifest format, examples, and an agent-ready setup guide for Codex or Claude Code.

## macOS launcher shortcut

On macOS, `rayon` tries to register `Command+Space` as the launcher shortcut. That is the same default shortcut used by Spotlight, so pressing it can open both Spotlight and `rayon`.

### Fix the Spotlight conflict

1. Open `System Settings > Keyboard > Keyboard Shortcuts`.
2. Open `Spotlight`.
3. Disable `Show Spotlight search`, or change it to a different shortcut.
4. Close System Settings and relaunch `rayon`.

If `Command+Space` is still unavailable, `rayon` also tries `Command+Shift+Space` as a fallback.

### Make only `rayon` appear

`rayon` is already configured as an accessory-style macOS app:

- it hides the Dock icon
- it stays out of the taskbar
- it opens as the launcher window only

If Spotlight still appears together with `rayon`, the issue is the macOS keyboard shortcut conflict above. Once Spotlight is disabled or remapped, pressing the `rayon` shortcut should only show the `rayon` launcher.
