# Adding A Feature

Rayon is intentionally small.

When you add a feature, the main goal is usually:

1. Make something searchable.
2. Return a typed result to the UI.
3. Run the right action when the user selects it.

## Quick map

- `apps/desktop/`: frontend UI
- `apps/desktop/src-tauri/`: desktop app host and wiring
- `crates/types/`: shared types passed between backend and UI
- `crates/core/`: command registry, search orchestration, execution routing
- `crates/features/`: built-in launcher features
- `crates/platform/`: OS-specific behavior
- `crates/db/`: Tantivy search index

## The usual feature paths

### Add a built-in launcher feature

Use this when the feature behaves like a command inside Rayon.

Typical flow:

1. Add or update shared types in `crates/types` if needed.
2. Add a provider in `crates/features`.
3. Register it in `crates/features/src/lib.rs`.
4. If it needs OS access, call through `AppPlatform`.
5. If it needs to appear in search, expose command metadata through the provider.

Good fit:

- clipboard actions
- process actions
- small integrations
- interactive pickers

### Add platform behavior

Use `crates/platform` when the feature talks directly to macOS tools or system APIs.

Keep it narrow:

- discover data
- launch or focus something
- run a system action

Then call it from `crates/features` or `crates/core` through `AppPlatform`.

### Add frontend-only behavior

If the feature is mostly presentation, state, or interaction in the window, keep it in `apps/desktop/`.

Do not push UI-only concerns into the Rust crates unless the backend truly needs to know about them.

## Simple rules

- Prefer extending an existing provider before adding a new layer.
- Prefer `crates/features` for product behavior and `crates/platform` for system behavior.
- Keep volatile app-host concerns in `apps/desktop/src-tauri/`.
- Add shared types only when both frontend and backend need them.
- If a file starts becoming hard to scan, split it by responsibility instead of growing it further.

## A good default workflow

1. Decide whether the feature is frontend-only, built-in backend behavior, or platform integration.
2. Add the smallest shared type surface you need.
3. Wire the feature into search and execution.
4. Add tests close to the code you changed.
5. Keep the final shape obvious to the next person reading it.
