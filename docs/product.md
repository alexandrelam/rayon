# Product Description

## Overview

`rayon` is a Raycast-like desktop launcher built with Rust and Tauri.

The product goal is a fast, local-first, keyboard-driven launcher that grows into a plugin-oriented command platform.

## Product Direction

- keyboard-first at all times
- local-first by default
- low-latency interaction
- command-centric instead of window-centric

The command palette is the primary interface: type, search, select, execute.

## Platform Goals

- macOS is the first target
- the code structure should not block Linux later
- OS-specific behavior should stay outside core command logic

## Architecture Goals

- Rust owns the core logic
- the Tauri frontend is a thin UI shell
- built-in modules and external plugins should fit the same command model
- searchable records should be indexed through Tantivy
- plugin loading should not require frontend API changes

## Current State

- built-in providers exist
- command execution is generic
- Tantivy is integrated for app indexing
- contributor plugins are not implemented yet

## User Experience Principles

- typing is always focused
- no mouse is required
- arrow keys move through results
- `Enter` executes the selected command
- `Escape` closes or backs out of the current transient state
- focus states are explicit and visible
- perceived latency should be near-instant

## Long-Term Shape

Over time, `rayon` should become a general command platform:

- built-in features and plugins register searchable items
- Tantivy indexes those items
- execution stays behind a generic command interface

The core interaction model should stay unchanged: open palette, type, select, execute.
