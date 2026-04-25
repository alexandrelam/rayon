# Product Description

## Overview

`rayon` is a Raycast-like desktop launcher built with Rust and Tauri.

The product goal is a fast, local-first, keyboard-driven command launcher that starts with a minimal command palette and grows into an extensible system for built-in features and future extensions.

## Product Direction

The launcher should feel immediate and focused:

- keyboard-first at all times
- local-first by default
- low-latency interaction
- minimal visual noise
- command-centric instead of window- or mouse-centric

The command palette is the primary interface. The user types, sees matching commands, navigates with arrow keys, and executes with `Enter`. The product should feel closer to a terminal or editor command bar than a traditional desktop app UI.

## Platform Goals

- macOS is the first target
- the code structure should not block Linux later
- OS-specific behavior should stay isolated from core command logic

## Architecture Goals

- Rust owns the core logic
- the Tauri frontend is a thin UI shell
- the codebase stays modular and extensible
- built-in features and future extensions should fit the same command system shape
- avoid premature abstractions, but keep the right boundaries early

## Milestone 1

The first milestone is intentionally small. It exists to validate the architecture and the interaction loop before adding search indexing or richer features.

Current scope:

- command palette UI
- one built-in `hello` command
- generic frontend-to-backend command flow
- in-memory search
- command execution returning a simple result string

Out of scope for this milestone:

- app indexing
- persistence
- plugin runtime
- clipboard history
- process management
- global shortcuts
- routing-heavy UI
- external search systems

## User Experience Principles

- typing is always focused
- no mouse is required
- arrow keys move through results
- `Enter` executes the selected command
- `Escape` closes or backs out of the current transient state
- focus states are explicit and visible
- perceived latency should be near-instant

## Long-Term Shape

Over time, `rayon` should grow from a simple launcher into a general command platform:

- built-in features implemented as command providers
- future extensions using the same conceptual interface
- replaceable search/indexing implementation
- optional platform-specific capabilities layered under the command system

The key constraint is that growth should not compromise the core interaction model: open palette, type, select, execute.
