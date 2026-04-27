# Rayon Backend Architecture

## Scope

This document describes the Rust backend that powers Rayon today: how the workspace is structured, which crates depend on which others, how the desktop shell composes them, and how search/execution flows through the system.

It focuses on the **current implementation**, not only the intended direction described in [`README.md`](../README.md) and [`docs/project-structure.md`](./project-structure.md).

## High-level shape

Rayon is a Rust workspace plus a Tauri desktop shell.

- The Rust workspace provides reusable backend crates under `crates/`.
- The desktop app in `apps/desktop/src-tauri/` is the composition root that wires those crates together and exposes them to the frontend through Tauri commands.
- The backend is currently macOS-first. Platform integration is implemented by `MacOsAppManager` and macOS-specific clipboard/window behavior.

At a high level, the backend is organized around one core idea:

1. Gather searchable things.
2. Index them into a shared search backend.
3. Return typed search results to the UI.
4. Route execution back to the correct subsystem when the user selects a result.

## Workspace layout

```text
apps/desktop/src-tauri    Tauri host app and runtime composition root
crates/types              Shared domain and IPC types
crates/db                 Tantivy-backed search index
crates/platform           macOS discovery and OS actions
crates/core               Launcher orchestration, registry, config loading
crates/features           Built-in command providers
```

## Dependency graph

### Direct crate dependencies

```text
rayon-types
├─ no workspace crate dependencies

rayon-db
└─ rayon-types

rayon-platform
└─ rayon-types

rayon-core
├─ rayon-db
├─ rayon-platform
└─ rayon-types

rayon-features
├─ rayon-core
└─ rayon-types

rayon-desktop (Tauri app)
├─ rayon-core
├─ rayon-db
├─ rayon-features
├─ rayon-platform
└─ rayon-types
```

### Dependency interpretation

- `rayon-types` is the base contract crate. It defines the shared language used across search, execution, interactive sessions, browser tabs, apps, bookmarks, and theme state.
- `rayon-db` is an infrastructure crate. It depends only on shared types and Tantivy.
- `rayon-platform` is another low-level infrastructure crate. It depends only on shared types and wraps macOS/CLI behavior.
- `rayon-core` is the orchestration layer. It depends on both infrastructure crates and on shared types.
- `rayon-features` sits above `rayon-core`. It implements built-in commands using core traits plus platform access where needed.
- `rayon-desktop` is intentionally the most concrete layer. It owns app startup, state, persistence paths, Tauri commands, tray/shortcut behavior, and the final wiring of the backend.

## What each crate owns

## `crates/types`

`rayon-types` defines the shared backend contract.

Key responsibilities:

- Command identity and metadata: `CommandId`, `CommandDefinition`, `CommandArgumentDefinition`
- Search result payloads: `SearchResult`, `SearchResultKind`
- Execution payloads: `CommandExecutionRequest`, `CommandExecutionResult`, `CommandInvocationResult`
- Interactive command/session protocol: `InteractiveSession*`
- Theme preference model: `ThemePreference`
- Platform-domain models: `InstalledApp`, `BrowserTab`, `BrowserTabTarget`, `ProcessMatch`
- Shared indexing document format: `SearchableItemDocument`

This crate is effectively the ABI between:

- core orchestration
- built-in providers
- platform services
- the Tauri boundary
- the frontend invoke layer

Because the desktop app returns these types directly from Tauri commands, `rayon-types` is both a backend domain crate and an IPC contract crate.

## `crates/db`

`rayon-db` wraps Tantivy as Rayon’s shared text search engine.

Key responsibilities:

- Create/open a persistent Tantivy index on disk
- Create an in-memory Tantivy index for temporary flows
- Replace the full searchable corpus with `replace_items`
- Search for matching item ids with `search_item_ids`
- Rebuild the index directory if Tantivy reports a schema mismatch

Important implementation details:

- The index schema stores only the minimal fields needed for lookup: id, kind, title, subtitle, owner plugin id, and combined search text.
- Search returns item ids, not full result objects. Higher layers keep ownership of the canonical metadata and reconstruct `SearchResult` values afterward.
- Prefix search support is implemented manually by expanding tokens into prefixes before indexing. That is why short incremental queries work without custom Tantivy analyzers.

There are two distinct uses of this crate today:

- The main launcher search index, persisted on disk in app-local data.
- A secondary in-memory index used only for browser tab caching/search in `AppState`.

## `crates/platform`

`rayon-platform` provides macOS-specific discovery and execution primitives through `MacOsAppManager`.

Key responsibilities:

- Discover installed applications
- Launch an application with `/usr/bin/open`
- Open bookmark URLs
- Enumerate Chrome tabs through AppleScript
- Focus a specific Chrome tab
- Search processes by name or port
- Terminate processes with `kill -TERM`

This crate is intentionally imperative and OS-facing. It shells out to macOS tools such as:

- `mdfind`
- `open`
- `osascript`
- `ps`
- `lsof`
- `kill`

The launcher does not talk to those tools directly. It talks to the `AppPlatform` trait from `rayon-core`, and `MacOsAppManager` is the production implementation of that trait.

## `crates/core`

`rayon-core` is the backend’s central orchestration layer.

It owns four major concerns:

1. Command provider registration and dispatch
2. Config-driven command/bookmark loading
3. Search corpus assembly
4. Launcher runtime behavior and execution routing

### 1. Command provider system

The main abstraction is `CommandProvider`.

Each provider can:

- declare commands
- execute a command directly
- optionally start an interactive session
- optionally search and submit interactive session state

`CommandRegistry` is the in-memory router for these providers.

It stores:

- providers
- registered commands
- command id to provider ownership mapping

The registry turns provider metadata into searchable `SearchResult` values and searchable `SearchableItemDocument` values. That means providers are the source of truth for command metadata, while the search index is only an acceleration layer.

### 2. Config loading

The config subsystem loads manifests from:

- `$XDG_CONFIG_HOME/rayon`
- or `~/.config/rayon`

Each `.toml` file is parsed as a plugin-style manifest containing:

- `plugin_id`
- `[[commands]]`
- `[[bookmarks]]`

Today, config loading produces:

- declarative command providers backed by shell execution
- bookmark definitions

The loaded command provider is `DeclarativeCommandProvider`. It converts manifest commands into executable specs that:

- resolve relative paths relative to the manifest directory
- append runtime argv to configured base args
- optionally set working directory and environment variables
- run the configured program as a subprocess

This is the concrete realization of the repo’s “plugin-like” direction today: not a general runtime, but a manifest-based extension surface for commands and bookmarks.

### 3. Search assembly

`LauncherService::reindex_search` builds the shared search corpus from three sources:

- registered command providers
- discovered application catalog
- loaded bookmarks

That combined corpus is converted to `SearchableItemDocument` values and pushed into the shared `SearchIndex` trait.

This is an important design choice: search is unified at the launcher layer, not owned separately by each subsystem.

### 4. Launcher runtime

`LauncherService` is the main backend service object.

It owns:

- the command registry
- the platform adapter
- the shared search index
- the discovered app catalog
- the bookmark catalog
- active interactive sessions
- a session id counter

At construction time it:

1. asks the platform to discover apps
2. stores those apps in `AppCatalog`
3. builds bookmark state
4. attempts to reindex the shared search corpus

Execution routing inside `LauncherService` is explicit and layered:

- `apps.reindex` is handled specially by the launcher itself
- ids starting with `app:macos:` are treated as app launches
- browser-tab ids are parsed and routed to the platform focus action
- bookmark ids are opened as URLs
- provider-owned interactive commands start sessions through the registry
- everything else is delegated to the owning provider

So the launcher is not just a thin facade. It is the policy layer that decides which result kinds are launcher-native and which are provider-owned.

## `crates/features`

`rayon-features` contains built-in command providers that plug into `rayon-core`.

Current built-ins:

- `ClipboardHistoryProvider`
- `GitHubMyPrsProvider`
- `KillProvider`
- `MaintenanceProvider`
- `ThemeCommandProvider`

These are assembled through `built_in_providers(...)`, which receives runtime dependencies from the desktop app:

- clipboard history service
- platform adapter
- theme settings store

This crate is significant because it shows the intended extension model in practice:

- built-ins are packaged as providers
- providers declare commands through the same registry mechanism as user config commands
- interactive features use the same session protocol as any future provider would

### Provider split

The built-in providers fall into two categories.

#### Direct command providers

- `MaintenanceProvider`

These return a normal command result immediately.

#### Interactive session providers

- `ClipboardHistoryProvider`
- `GitHubMyPrsProvider`
- `KillProvider`
- `ThemeCommandProvider`

These start a session, let the user search/select within that session, and then either:

- complete the session, or
- update the session with a new result set/message

That interactive-session path is one of the most important architectural patterns in the backend.

## `apps/desktop/src-tauri`

This crate is the composition root and application host.

It owns:

- Tauri startup
- global shortcut registration
- tray creation
- launcher window visibility/focus behavior
- app-local path resolution
- long-lived `AppState`
- Tauri command handlers that bridge frontend requests to the Rust backend

This layer is where the system becomes a desktop application rather than just a backend library.

## Runtime composition

## Startup sequence

On startup, the Tauri app does the following:

1. Build a persistent Tantivy index in app-local data.
2. Create the macOS platform adapter.
3. Create clipboard access and clipboard history persistence.
4. Create theme settings persistence.
5. Build a `LauncherService` by wiring platform, search index, built-in providers, and config-loaded providers together.
6. Spawn the clipboard watcher thread.
7. Register tray behavior and global shortcut handling.
8. Expose Tauri invoke commands for search, execution, sessions, and preferences.

The `AppState` type is the desktop app’s runtime hub. It owns:

- `LauncherService`
- the platform adapter
- the shared search index
- clipboard history service
- theme settings store
- a browser-tab search cache

## Why `AppState` exists above `LauncherService`

`LauncherService` is the generic launcher backend, but some concerns are app-host-specific:

- persistence paths come from Tauri app directories
- clipboard watcher lifecycle is process-local
- browser tab caching uses an app-local mutex and in-memory index
- reloading swaps the entire launcher instance in place

That is why `AppState` wraps `LauncherService` instead of pushing everything into `rayon-core`.

## Main backend flows

## 1. Aggregate search flow

Normal launcher search works like this:

1. `AppState::search` calls `LauncherService::search`.
2. `LauncherService` queries the shared `SearchIndex` for matching item ids.
3. It rebuilds an in-memory lookup map from:
   - registry command results
   - discovered apps
   - bookmarks
4. It returns the results matching the ids returned by Tantivy, preserving search ordering from the index.

Important consequence:

- Tantivy stores searchability and ordering.
- Core stores the canonical metadata.
- The index is therefore disposable/rebuildable.

## 2. Browser tab search flow

Browser tab search is handled separately from normal aggregate search.

Flow:

1. The frontend calls the dedicated `search_browser_tabs` Tauri command.
2. `AppState` checks or refreshes `BrowserTabSearchCache`.
3. Refresh pulls live tab data from the platform layer.
4. The cache indexes those tabs into an in-memory Tantivy index.
5. Results are returned as `SearchResultKind::BrowserTab`.

This means browser tabs are **not** part of the main persistent search corpus. They are a separate volatile search lane.

That separation makes sense because browser tabs are:

- short-lived
- refresh-heavy
- platform-derived at request time

## 3. Command execution flow

Execution starts with the Tauri command `execute_command`.

From there:

1. Reindex requests trigger `AppState::reload`, which rebuilds the launcher and then runs `apps.reindex`.
2. Other requests go to `AppState::execute_command`.
3. `LauncherService::execute_command` dispatches by command/result kind:
   - launcher-native maintenance
   - installed app launch
   - browser tab focus
   - bookmark open
   - interactive provider session start
   - normal provider execution

This is effectively a command bus with special-case routing for result types that are not pure provider commands.

## 4. Interactive session flow

Interactive commands use a two-phase protocol:

1. The initial command execution starts a session and returns `StartedSession`.
2. The frontend issues follow-up search and submit calls against `session_id`.

`LauncherService` keeps active sessions in memory, keyed by generated ids such as `session-1`, `session-2`, and so on.

Each session stores:

- provider ownership
- command metadata
- completion behavior

The registry then routes session search/submit calls back to the original provider.

This is how clipboard history, GitHub PR search, kill-process, and theme selection all work without each feature having to invent a separate frontend protocol.

## Persistence model

Current persisted backend state is split across a few locations:

- Main search index: app-local data directory, under `search/apps`
- Theme settings: app-local data directory, under `settings/theme.json`
- Clipboard history: app-local data directory, under `settings/clipboard-history.json`
- User command/bookmark manifests: XDG config directory under `rayon/*.toml`

Notably:

- discovered apps are not persisted independently; they are rediscovered and reindexed
- interactive sessions are in-memory only
- browser tab cache is in-memory only

## How the system is actually “plugin-like”

The docs describe a future plugin direction. In the current code, that direction appears in a narrower form:

- user manifests act like lightweight plugins
- each manifest has a `plugin_id`
- commands/bookmarks are namespaced by that plugin id
- built-in functionality is also expressed as command providers

What is **not** present yet:

- no general dynamic plugin runtime
- no separate extension process model
- no runtime loading of arbitrary Rust plugins
- no unified provider registration API for external compiled modules

So today’s architecture is better described as:

> a launcher core with built-in providers plus config-defined command/bookmark extensions

not yet:

> a full plugin platform

## Boundaries and layering

The main architectural boundaries are clean:

- `types` defines contracts
- `platform` talks to the OS
- `db` talks to the search engine
- `core` owns orchestration and policy
- `features` implements built-in commands
- `desktop` wires everything into an app

There are also two places where the current layering is intentionally pragmatic rather than pure:

1. `rayon-core` depends on concrete crates `rayon-db` and `rayon-platform`, even though it also defines traits (`SearchIndex`, `AppPlatform`) that abstract them.
2. Browser-tab search logic lives in desktop `AppState` rather than in `rayon-core`, because it is treated as a volatile UI-adjacent cache rather than a first-class member of the main search corpus.

Those are not necessarily problems; they just mean the project is optimizing for a working desktop launcher over a fully abstract architecture.

## End-to-end mental model

If you want one concise way to understand the system, it is this:

```text
Frontend UI
  -> Tauri invoke commands
  -> AppState
  -> LauncherService
     -> CommandRegistry + BookmarkCatalog + AppCatalog
     -> SearchIndex (Tantivy)
     -> AppPlatform (macOS)
     -> Built-in providers / declarative providers
```

And the data sources feeding that pipeline are:

```text
macOS app discovery
user TOML manifests
built-in provider definitions
live Chrome tabs
live process list / lsof
clipboard persistence
theme settings persistence
```

## Current-state summary

The backend today is a layered launcher engine with:

- a shared typed contract crate
- a Tantivy search backend
- a macOS platform adapter
- a central launcher orchestration service
- provider-based built-in features
- manifest-defined custom commands and bookmarks
- a Tauri host app that owns runtime state and UI bridging

The most important structural idea is that **search and execution are unified across apps, bookmarks, and commands**, but **browser tabs and interactive sessions are handled as separate runtime channels when they need more dynamic behavior**.

That combination explains most of the repo structure and most of the dependencies in the Rust backend.

## Source map

If you want to trace the implementation directly, these are the most important backend files:

- `apps/desktop/src-tauri/src/lib.rs`: Tauri bootstrap and command registration
- `apps/desktop/src-tauri/src/app/state.rs`: long-lived app runtime state and browser-tab cache
- `apps/desktop/src-tauri/src/app/launcher.rs`: launcher construction/reload path
- `apps/desktop/src-tauri/src/invoke/launcher.rs`: frontend-to-backend command bridge
- `crates/core/src/commands.rs`: provider trait and command registry
- `crates/core/src/launcher/service.rs`: launcher service construction
- `crates/core/src/launcher/search.rs`: aggregate search and reindex behavior
- `crates/core/src/launcher/execution.rs`: command execution routing
- `crates/core/src/launcher/sessions.rs`: interactive session flow
- `crates/core/src/config/mod.rs`: config entrypoint
- `crates/core/src/declarative_provider.rs`: config-defined command execution
- `crates/db/src/lib.rs`: Tantivy search backend
- `crates/platform/src/lib.rs`: macOS discovery and OS actions
- `crates/features/src/lib.rs`: built-in provider assembly
- `crates/types/src/lib.rs`: shared contracts used across all backend layers
