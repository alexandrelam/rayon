# Project Structure

## Target Shape

```text
apps/
└─ desktop/
   ├─ src/
   ├─ src-tauri/
   ├─ package.json
   └─ vite.config.ts

crates/
├─ core/                    # command contracts + orchestration
├─ features/                # built-in providers
├─ platform/                # OS integration
├─ db/                      # Tantivy-backed indexing
└─ types/                   # shared models

docs/
extensions/                 # future plugin area
```

## Responsibilities

### `apps/desktop`

Owns the shell only: UI, Tauri bootstrap, and generic invoke calls.

### `crates/types`

Owns the shared boundary between UI, core, and plugins.

### `crates/core`

Owns the command model, registration, search orchestration, and execution routing.

### `crates/features`

Owns built-in providers that use the same shape as future plugins.

### `crates/platform`

Owns platform-specific discovery and launch behavior.

### `crates/db`

Owns Tantivy-backed indexing and related storage concerns.

### `extensions`

Reserved for contributor plugins and extension manifests/runtime work.

## Architecture Notes

- Built-in modules and third-party plugins should register through the same backend contract.
- Tantivy should be the shared indexing layer for searchable items, not just OS apps.
- The frontend API should stay generic: `search(query)` and `execute_command(command_id, payload)`.

## Current State

- providers are still wired in statically
- Tantivy currently indexes discovered apps
- a general plugin runtime is not implemented yet
