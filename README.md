# rayon

Keyboard-first desktop launcher built with Rust, Tauri, and React.

## Layout

- `apps/desktop`: Tauri shell and command palette UI
- `crates/types`: shared serializable command models
- `crates/core`: command provider trait and registry orchestration
- `crates/features`: built-in command providers

## Commands

- `pnpm dev`: run the frontend dev server
- `pnpm tauri dev`: run the desktop app
- `cargo test`: run Rust tests across the workspace
