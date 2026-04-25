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
