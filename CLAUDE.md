# Traceflow — Claude Code context

## Stack
- Tauri v2 desktop app
- Rust 1.78+ backend (`src-tauri/`)
- React 18 + TypeScript frontend (`src/`)
- MiniJinja for text templates, docx-rs for .docx
- Event-sourced architecture — `events.ndjson` is source of truth

## Critical conventions
- All Tauri IPC commands in `src-tauri/src/commands.rs`, real logic in submodules
- Never mutate session state directly — append to the event log instead
- Every promoted frame is content-addressed: filename = SHA-256 of PNG bytes
- Hash chain integrity is non-negotiable — don't add code paths that mutate
  past records or skip the chain

## What's stubbed / not yet wired
- `privacy/redact.rs::apply()` is a no-op — needs OCR pass (Phase 2)
- `ai/describer.rs::describe()` is heuristic — needs local model (Phase 2)
- `MouseClick`, `KeyboardInput`, `WindowFocusChanged` event variants exist
  in the schema but no producers emit them yet (Phase 2)
- Accessibility-API capture source is not implemented (Phase 2)

## Error handling
- `anyhow::Result` in bin/lib code, propagate with `?`
- `thiserror` for library errors that cross module boundaries
- No `unwrap()` in production paths — only in tests

## Testing
- Unit tests live alongside code (`#[cfg(test)] mod tests`)
- Run with `cargo test --manifest-path src-tauri/Cargo.toml`

## Before any structural change
- Read the README architecture section
- Check whether the change should be a new event variant rather than
  a schema modification (event log is append-only and forward-compatible)