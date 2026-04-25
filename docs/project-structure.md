# Project Structure And Stack

## Stack

The current stack is intentionally small:

- Rust for core application logic
- Tauri for the desktop shell and frontend/backend bridge
- React for the command palette UI
- TypeScript for predictable frontend state and typed invoke calls
- Vite for frontend development and build tooling
- pnpm for JavaScript workspace/package management
- Cargo workspaces for Rust crate organization

## High-Level Folder Structure

```text
apps/
└─ desktop/
   ├─ src/                  # React UI shell
   ├─ src-tauri/            # Tauri app assembly and invoke commands
   ├─ package.json
   └─ vite.config.ts

crates/
├─ core/                    # command registry + orchestration
├─ features/                # built-in command providers
├─ platform/                # future OS abstraction boundary
├─ db/                      # future storage boundary
└─ types/                   # shared Rust models

docs/                       # product and architecture references
extensions/                 # future extension/plugin area
```

## Responsibilities

### `apps/desktop`

Owns the desktop application shell:

- React command palette UI
- Tauri window/app bootstrap
- generic frontend-to-backend invoke calls
- no feature-specific business logic

The frontend should stay thin. It renders state and forwards generic actions to Rust.

### `crates/types`

Owns shared serializable domain models such as:

- `CommandId`
- `CommandDefinition`
- `SearchResult`
- command execution result payloads

These types define the stable boundary between orchestration and transport.

### `crates/core`

Owns the command system itself:

- `CommandProvider` trait
- `CommandRegistry`
- registration rules
- search orchestration
- execution routing

This crate is the center of the backend architecture.

### `crates/features`

Owns built-in features implemented as command providers.

For milestone 1, this contains the `hello` provider. As the app grows, new built-in capabilities should be added here without changing the frontend API shape.

### `crates/platform`

Reserved for future OS-specific integrations such as:

- app launching
- system APIs
- platform-specific behavior

This boundary keeps platform details out of core orchestration.

### `crates/db`

Reserved for future persistence and storage concerns such as:

- cached indexes
- command history
- user settings

This is intentionally unused in the first milestone.

### `extensions`

Reserved for future extension/plugin work. The current milestone does not implement an extension runtime, but this folder makes the intended product direction explicit.

## Infra And Folder Strategy

The structure is designed around boundaries, not around frameworks.

### 1. Rust-first core

Business logic lives in Rust, not in React components and not in Tauri command handlers.

That means:

- command definitions live in Rust
- command search lives in Rust
- command execution lives in Rust
- Tauri commands are thin wrappers over core APIs

### 2. Thin frontend shell

The frontend should only know how to:

- capture keyboard input
- render results
- track selection
- call generic backend commands
- display execution output

The frontend should not know about feature-specific endpoints.

### 3. Generic command API

The public frontend/backend interface is deliberately small:

- `search(query)`
- `execute_command(command_id, payload)`

This keeps built-in features and future extensions aligned behind the same execution model.

### 4. Workspace-first repo layout

The repo is structured early as a multi-package workspace so future growth does not require another large migration.

- frontend app code lives under `apps/`
- reusable Rust logic lives under `crates/`
- docs live under `docs/`
- future extension work has a dedicated top-level area

### 5. Replaceable internals

The current implementation uses simple in-memory search. That is intentional.

The architecture should allow later replacement of:

- search implementation
- platform integrations
- storage layers
- extension loading

without changing the core UI contract.

## Current Milestone Shape

Today the project supports one vertical slice:

- load built-in providers
- search available commands
- select a result from the palette
- execute the selected command
- render the returned output

That slice is the baseline the rest of the product will build on.
