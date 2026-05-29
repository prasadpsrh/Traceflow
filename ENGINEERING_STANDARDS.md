# Traceflow Engineering Standards

> **Status:** Mandatory.
> **Applies to:** All Rust, TypeScript, and React code in the Traceflow repository.
> **Last updated:** 2026-05-29
> **Owner:** Lead Architect

This document is the single source of truth for *how we write code* in Traceflow. It is not a style guide alone — it encodes architectural decisions that protect the product's correctness, privacy, and tamper-evidence guarantees. Deviating from it is allowed only with an explicit, written exception in the PR description.

When in doubt: re-read §2 (Architectural Invariants). Every other rule in this document derives from those.

---

## Table of contents

1. [Core principles](#1-core-principles)
2. [Architectural invariants](#2-architectural-invariants-non-negotiable)
3. [Rust standards](#3-rust-standards)
4. [TypeScript & React standards](#4-typescript--react-standards)
5. [Error handling](#5-error-handling)
6. [Logging & observability](#6-logging--observability)
7. [Testing](#7-testing)
8. [Security & privacy](#8-security--privacy)
9. [Performance](#9-performance)
10. [Cross-platform support](#10-cross-platform-support)
11. [Dependencies](#11-dependencies)
12. [Git workflow & commits](#12-git-workflow--commits)
13. [Code review](#13-code-review)
14. [Documentation](#14-documentation)
15. [Tooling & automation](#15-tooling--automation)
16. [Working with Claude Code / agents](#16-working-with-claude-code--agents)
17. [Anti-patterns](#17-anti-patterns-to-avoid)
18. [Exceptions process](#18-exceptions-process)

---

## 1. Core principles

These are the values every decision should serve. When rules conflict, the principle higher on this list wins.

1. **Correctness over speed.** A wrong result delivered fast is worse than no result. Especially true for the hash chain and redaction pipeline.
2. **Honesty over polish.** A feature labeled "Windows-only" that works is better than a feature labeled "cross-platform" that silently no-ops.
3. **Privacy is a feature, not a setting.** The user's screen content is the most sensitive data we touch. Default behaviors must protect it.
4. **The event log is the truth.** State in memory is a cache; state on screen is a render. Both can disagree with the log. The log wins.
5. **Configuration over code for industry specifics.** If a feature applies to one industry only, it belongs in a rule pack or template, not in Rust.
6. **Test the contract, not the implementation.** A passing test should still pass after a refactor.
7. **Small, reviewable changes.** A 200-line PR is reviewed; a 2,000-line PR is rubber-stamped.

---

## 2. Architectural invariants (non-negotiable)

Breaking any of these is a release-blocker bug. They exist because the product's value depends on them.

### 2.1 The hash chain is sacred

- Every event written to a session log must carry a valid `prev` and `hash`.
- No code path may mutate an existing record. Append-only, always.
- No code path may skip writing an event when one should be written.
- The chain verification path (`events::chain::verify_chain`) must remain a pure function.
- Tests in `tests/integration.rs` under `chain_*` are regression guards. They must always pass.

### 2.2 Frames are content-addressed

- Frame PNG filenames are `{sha256_of_png_bytes}.png`. Never anything else.
- The hash is computed **after** any redaction is applied. The on-disk artifact and the chain attest to the same bytes.
- Never write a frame whose hash doesn't match its filename. Never read a frame and trust its filename.

### 2.3 The event log is the source of truth

- The React UI must never display state that doesn't have a corresponding event in the log.
- After mutations (delete step, edit description), re-fetch from the log via `get_session_steps`. Do not mutate the local cache and skip the round-trip.
- `AppState` in Rust is a *cache and coordinator*, not a store. If the cache disagrees with the log, the log wins.

### 2.4 Industry specifics live in config, not code

- No hardcoded industry patterns in Rust. (No "if this looks like a Salesforce window…")
- No hardcoded industry vocabulary in templates. (No "Patient ID" in default templates.)
- All industry behavior comes from rule packs (JSON) and templates (Jinja / JSON specs).

### 2.5 OCR text never persists

- OCR results live in memory only.
- The event log stores `text_hash` (SHA-256) and bounding boxes, never the recognized text itself.
- Any code path that adds plain-text OCR output to a serialized struct is rejected at review.

### 2.6 The capture path runs offline

- No network calls anywhere in the capture, redaction, document, or events modules.
- No "phone home" telemetry. No anonymous usage analytics. No update checks during capture.
- If a future feature needs a network, it goes in a separate, opt-in module that is **off by default** and clearly disclosed to the user.

---

## 3. Rust standards

### 3.1 Edition, version, toolchain

- Rust **edition 2021** unless otherwise specified in `Cargo.toml`.
- MSRV (minimum supported Rust version): pinned in `rust-toolchain.toml`. Bumping it requires a maintainer-level review.
- Use the workspace-pinned versions in `Cargo.toml`. Do not add `version = "*"` or unbounded ranges.

### 3.2 Formatting

- `rustfmt` with the default `rustfmt.toml` (or whatever is checked in). No manual formatting.
- CI must run `cargo fmt --check` and fail on diffs.
- Always run `cargo fmt` before pushing.

### 3.3 Linting

- `cargo clippy --all-targets --all-features -- -D warnings` must pass.
- Allowed lint overrides require a `// reason: ...` comment immediately above the `#[allow(...)]`.
- Common allowed exceptions:
  - `#[allow(dead_code)]` is forbidden in production modules. Either use the code or delete it.
  - `#[allow(clippy::too_many_arguments)]` is allowed only for IPC commands with a documented payload struct as the better alternative.

### 3.4 Module organization

- One concept per module. If a `.rs` file exceeds 500 lines, split it.
- `mod.rs` declares submodules and re-exports the public API. No business logic in `mod.rs`.
- IPC command handlers live only in `commands.rs` (or `commands/<area>.rs` if split). Real logic always lives in submodules.

### 3.5 Naming

- Modules: `snake_case`. Match the domain term, not the implementation. `events::log` not `events::ndjson_writer`.
- Types: `PascalCase`. Concrete, noun-form: `EventLog`, `RuleEngine`, not `EventLogManager`, `RuleEngineHandler`.
- Functions: `snake_case`. Verb-form: `append`, `verify_chain`, not `do_append`, `chain_verifier`.
- Constants: `SCREAMING_SNAKE_CASE`. Document the unit in the name when applicable: `MAX_FRAME_BYTES`, `DEFAULT_POLL_FPS`.
- Lifetimes: short and meaningful — `'a` for one-off, `'session` or `'log` when scope conveys meaning.

### 3.6 Error handling

See §5 for the full policy. Quick summary for Rust:

- Use `anyhow::Result<T>` for binary / library composition code that propagates errors.
- Use `thiserror` to define library errors that cross module boundaries.
- IPC command signatures: `Result<T, String>` (Tauri serializes to JS).
- Never `unwrap()` or `expect()` in production paths. Allowed only in tests, benches, and `main.rs` startup before logging is configured.
- Always include context. `.context("opening session log")?` is required when the bare error would not be self-explanatory.

### 3.7 Async & concurrency

- Use `tokio` as the only async runtime. Do not introduce `async-std`, `smol`, or other runtimes.
- Spawn long-running background tasks with `tokio::spawn`. Never block the runtime with synchronous I/O on its threads.
- CPU-heavy work (PNG encoding, hashing, OCR) goes in `tokio::task::spawn_blocking`. Never on the runtime threads.
- Shared mutable state: `tokio::sync::Mutex` (not `std::sync::Mutex`) when held across `.await` points.
- Channels: `tokio::sync::mpsc` for owned-producer-consumer, `tokio::sync::broadcast` for fan-out.
- Never use `block_on` from inside an async function. Never call `.unwrap()` on a `JoinHandle` without handling cancellation.

### 3.8 Unsafe code

- Forbidden in `traceflow_lib` source unless absolutely necessary for FFI (e.g. Windows COM interop).
- Every `unsafe` block requires:
  1. A `// SAFETY:` comment explaining why the operation is sound.
  2. Encapsulation: the smallest possible block, never an entire function.
  3. A safe wrapper exposed to callers.
- Add `#![deny(unsafe_op_in_unsafe_fn)]` and `#![warn(clippy::undocumented_unsafe_blocks)]` to module roots that need unsafe.

### 3.9 Public API surface

- `pub` is a commitment. Use `pub(crate)` or `pub(super)` when scope allows.
- Every `pub` item in the library crate must have a doc comment.
- Breaking changes to `pub` items in `traceflow_lib` require a version bump and a `CHANGELOG.md` entry.

### 3.10 Trait usage

- Don't introduce a trait for a single implementor. Wait until there are two.
- Use trait objects (`dyn Trait`) for plugin-style polymorphism (e.g. `CaptureSource`).
- Use generics for performance-critical paths where monomorphization wins.
- Implement `Debug` for all public types unless there's a security reason not to (e.g. anything holding decrypted credentials).

### 3.11 Serialization

- `serde` with explicit field renames where the JSON / TOML format must remain stable: `#[serde(rename = "...")]`.
- Use `#[serde(tag = "kind", rename_all = "snake_case")]` for enums in event logs, configs, and IPC payloads.
- Backward compatibility: new fields must be `Option<T>` or have `#[serde(default)]`. Never remove or reorder enum variants — old logs must still deserialize.

---

## 4. TypeScript & React standards

### 4.1 TypeScript settings

- `strict: true` in `tsconfig.json`. No exceptions.
- `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch` all enabled.
- Never use `any`. Use `unknown` and narrow.
- Type assertions (`as Foo`) require a comment explaining why the narrowing is safe.

### 4.2 Component conventions

- One component per file. File name matches default export.
- Functional components with hooks only. No class components.
- Props interfaces named `Props` (local to the file).
- Mirror Rust types in `src/types.ts`. Do not duplicate type definitions inside components.

### 4.3 State management

- Local state (`useState`) for component-internal concerns.
- Lifted state in `App.tsx` for cross-component coordination.
- No Redux, Zustand, MobX, or other state library until the team has explicitly agreed we need it.
- Server-state (data from Rust): always re-fetched from IPC after mutations. Do not optimistically update local state in ways that diverge from the event log.

### 4.4 IPC calls

- Use `invoke<T>("command_name", { args })` with explicit type parameters.
- Wrap every `invoke` in a `try/catch` that surfaces errors to the user with actionable language. Never silently ignore.
- Name IPC commands in `snake_case` to match the Rust side.
- Camel/snake case translation: Tauri auto-converts `snake_case` Rust → `camelCase` JS argument names. Be aware of this and pass `stepIndex`, not `step_index`, on the JS side.

### 4.5 Styling

- CSS variables defined in `styles.css` `:root`. No inline CSS variable definitions in components.
- Tailwind is **not** used (we use vanilla CSS with the editorial design system). Do not introduce it without team agreement.
- One CSS class per visual concern. Avoid `style={{...}}` inline styles except for dynamic values (e.g. progress widths).

### 4.6 Accessibility

- Every interactive element must be keyboard-reachable.
- Every form input must have a `<label htmlFor="...">` or `aria-label`.
- Modal dialogs must trap focus and restore on close. (Not yet implemented — flag it when you ship one.)
- Color contrast: meet WCAG 2.1 AA at minimum.

### 4.7 React-specific

- `useEffect` dependency arrays must be exhaustive. Run with `eslint-plugin-react-hooks/exhaustive-deps`.
- Cleanup functions in `useEffect` are mandatory for subscriptions, timers, and IPC listeners. The current code uses `let un: UnlistenFn | null = null;` then calls it on cleanup — keep that pattern.
- Avoid `useEffect` for derived state. Use `useMemo` or compute inline.

---

## 5. Error handling

### 5.1 Rust: the four cases

| Situation | Pattern |
|---|---|
| Recoverable error in app code | `anyhow::Result<T>` + `?` |
| Library boundary | `thiserror`-derived error enum |
| IPC command | `Result<T, String>` (Tauri serializes) |
| Truly impossible state | `unreachable!()` with a comment explaining why |

Never use `unwrap()` or `expect()` in production code paths. Tests are the only exception.

### 5.2 Rust: error messages

- Start with a lowercase verb: `"opening session log"`, not `"Could not open the session log."`
- Don't end with punctuation.
- Include the input that failed: `format!("parsing rule pack {}", path.display())`.
- Don't leak internal types into user-facing messages.

### 5.3 TypeScript: user-facing errors

- Every `invoke` failure surfaces to the user via a toast or alert with:
  1. What they tried to do ("Could not start the capture session.")
  2. What might be wrong ("Make sure another session isn't already running.")
  3. What to do next ("If this persists, check the application log.")
- Never just show the raw error string. Translate.

### 5.4 The "log and continue" anti-pattern

Forbidden in critical paths (capture, redaction, event log). If an event fails to append, the capture must **stop**, not continue with a broken chain.

Allowed in non-critical paths (telemetry, AI description, OCR enrichment) where the user still gets a valid output. Always emit a structured log when this happens.

---

## 6. Logging & observability

### 6.1 Use `tracing`, not `log`

- New code uses `tracing::{debug, info, warn, error}` macros.
- Existing `log::` calls are being migrated; leave them in place unless you're already touching the file.
- Add `#[tracing::instrument]` to functions that benefit from span-based timing.

### 6.2 Log levels

| Level | Use for |
|---|---|
| `error!` | Failures the user should know about. Always investigated. |
| `warn!` | Degraded but recovered. Worth seeing in logs. |
| `info!` | Lifecycle events: session start/stop, exports, settings changes. |
| `debug!` | Per-frame, per-event details. Off by default. |
| `trace!` | Hot-loop internals. Almost never enabled in production. |

### 6.3 What to log, what not to

**Always log:**
- Lifecycle: session start/stop, configuration loaded, plugin initialized
- Errors: with full context chain via `anyhow`
- Security-relevant: chain verification failures, rule pack loaded with N rules

**Never log:**
- OCR text content
- File paths inside the user's home directory at info level or above (mask to `<user>/Documents/Traceflow/...`)
- Window titles of third-party apps without redaction at info level or above
- Anything that could be PII

### 6.4 Structured fields

```rust
// Good
tracing::info!(session_id = %id, steps = step_count, "session ended");

// Bad
log::info!("session {} ended with {} steps", id, step_count);
```

Structured fields let logs be filtered and queried later. Format-string logs cannot.

---

## 7. Testing

### 7.1 Test pyramid

| Layer | Where | What |
|---|---|---|
| Unit | `#[cfg(test)] mod tests` inside the module | Pure-function logic: diff scoring, regex matching, chain hashing |
| Integration | `src-tauri/tests/integration.rs` | End-to-end: events → projection → export, with synthetic frames |
| UI smoke | `npm run test` (Vitest, when added) | Component renders, IPC mocks |
| Manual smoke | `SMOKE_TEST.md` (to be created) | Pre-release walkthrough |

### 7.2 What every test must do

- Be deterministic. No flaky time-based tests; use a fake clock if needed.
- Be hermetic. No reliance on real network, real display, real keyboard.
- Be fast. Unit tests under 100ms each, integration tests under 1s each.
- Be self-documenting. Test name describes the contract: `tampered_chain_fails`, not `test_chain_2`.

### 7.3 What tests are required for a PR

| Change type | Required tests |
|---|---|
| New IPC command | Integration test exercising it through `tauri::test` or a thin wrapper |
| New event variant | Round-trip serialize test + chain integrity test |
| Capture engine change | A test asserting frame promotion / non-promotion under specific conditions |
| Rule engine change | Pattern tests for at least one matching and one non-matching input |
| Export format change | Golden-file comparison test in `tests/integration.rs` |
| UI-only change | No Rust test required; visual review in PR is sufficient |

### 7.4 Golden files

- Stored under `src-tauri/tests/golden/`.
- Updated only when the change is reviewed and intentional.
- Update process: run with `UPDATE_GOLDEN=1 cargo test`, commit the diff with the rationale.

### 7.5 The `FakeSession` builder

- Use it for any test that touches the event log, capture engine, or document renderer.
- Do not bypass it by writing files directly — that diverges from the real production path.
- If you need a feature `FakeSession` doesn't support, extend it. Don't fork it.

---

## 8. Security & privacy

### 8.1 Threat model

Traceflow's primary threats, in order of severity:

1. **Tampering with audit evidence** — someone modifying a session log to falsify a record
2. **Leaking PII** — captured content escaping the local machine via misconfigured tools
3. **Code execution via crafted input** — a malicious rule pack or template attacking the parser
4. **Privilege escalation** — using Traceflow's screen-capture privileges to read content from another user

Every PR is reviewed against these threats.

### 8.2 Crypto rules

- Hashing: `sha2::Sha256` only. No MD5, SHA-1, or non-cryptographic hashes for security purposes (`xxhash` is fine for dedup-only contexts).
- Encryption (when added): `aes-gcm` with a 256-bit key and random 96-bit nonce. Never reuse nonces.
- Key derivation: `argon2` with the OWASP recommended parameters.
- Never roll your own crypto. Never assume "nobody will look" for weak crypto.

### 8.3 Input validation

Inputs that must be validated before use:

- **Rule pack JSON**: schema-validated, regex patterns compiled before save (we do this — keep doing it)
- **Template files**: parsed before activation, errors surface to the UI
- **User file paths** in IPC commands: never trust them, always canonicalize and check they're inside an allowed root (data root or user-picked path)
- **Regex patterns**: compile to verify; reject patterns with catastrophic backtracking potential (no easy automated check — code review responsibility)

### 8.4 Privacy defaults

- "Redact PII" defaults to **on** in new installs.
- Telemetry defaults to **off** (and we don't have telemetry — keep it that way unless explicitly proposed).
- Logs default to the data root, not the system temp directory.
- Frame retention defaults to 30 days; older frames pruned automatically.

### 8.5 The "principle of least privilege"

- Capture permissions requested only when capture starts, not at app launch.
- Filesystem access scoped via Tauri capabilities — see `capabilities/default.json`. Adding permissions requires PR review.
- No automatic outbound connections. If a future feature adds one, it requires explicit user opt-in per session.

---

## 9. Performance

### 9.1 Budgets

For a single typical capture frame on a target machine (Windows 11, Intel i5-12th gen, 16 GB RAM, 1080p display):

| Operation | Target | Hard ceiling |
|---|---|---|
| Frame grab + fingerprint | < 15 ms | 30 ms |
| Diff vs reference | < 2 ms | 5 ms |
| Promotion: encode PNG + hash + write | < 100 ms | 250 ms |
| OCR pass (when on) | < 250 ms | 500 ms |
| Rule evaluation over OCR text | < 5 ms | 20 ms |
| Document export (10-step session) | < 500 ms | 1 s |

If a change pushes any path past its hard ceiling, profile it and either optimize or document the regression in the PR.

### 9.2 Allocations

- Avoid allocating in the capture hot loop. Reuse buffers.
- `Vec::with_capacity` when the size is known.
- `String::with_capacity` for known-length string building.

### 9.3 What to profile

- New code in capture/, ocr/, document/ paths.
- Anything claiming to be "fast."
- Use `cargo flamegraph` or `tracing-tracy` for visualization. Commit profiling notes in the PR.

### 9.4 The "do it once" rule

Compiled artifacts (regex sets, templates, rule engines) are compiled once and cached. Re-compiling per frame is a release-blocker.

---

## 10. Cross-platform support

### 10.1 Honesty rule

- Code that's Windows-only must be gated with `#[cfg(target_os = "windows")]`.
- A no-op fallback for other platforms is required so the code compiles cross-platform.
- Features that are Windows-only must be **disabled in the UI** on other platforms, with a clear message.
- README, USER_MANUAL, and marketing copy must match the actual platform support matrix.

### 10.2 Adding a Windows-only feature

1. Implement under `#[cfg(target_os = "windows")]`.
2. Add a `#[cfg(not(target_os = "windows"))]` stub that returns a clear "not supported on this platform" error.
3. Add a runtime check in the IPC command that surfaces to the UI.
4. Update the platform matrix in `README.md`.
5. File a tracking issue for cross-platform support.

### 10.3 Test matrix

| Platform | CI required | Manual smoke |
|---|---|---|
| Windows 11 | ✅ | Before every release |
| macOS (latest) | ✅ | Before every release |
| Linux (Ubuntu LTS, X11) | ✅ | Before every release |
| Linux (Ubuntu LTS, Wayland) | ⚠️ degraded support — document gaps | Quarterly |

### 10.4 File paths

- Always use `std::path::PathBuf` and `Path`. Never `String` concatenation.
- Never hardcode `/` or `\` separators. Use `.join()`.
- Never assume `~` expansion. Use `dirs::home_dir()`.

---

## 11. Dependencies

### 11.1 Adding a dependency

Before adding any new crate or npm package, answer in the PR:

1. **What problem does it solve that we can't solve in <100 lines of our own code?**
2. **Is it actively maintained?** (Last commit < 12 months, > 100 stars or known authors)
3. **What's the license?** MIT, Apache-2.0, BSD-3 are fine. GPL, LGPL, AGPL require team review.
4. **What's the dependency tree size?** `cargo tree | wc -l` before and after.
5. **Is there a Rust-native alternative?** Prefer Rust libs over FFI wrappers when possible.

### 11.2 Pinning

- Production deps: pin to compatible range (`"^2.0"`).
- No `version = "*"` ever.
- `Cargo.lock` and `package-lock.json` are committed and authoritative.

### 11.3 Security

- `cargo audit` must pass with no high-severity advisories.
- `npm audit` must pass with no high-severity advisories.
- A dependency with an unpatched high-severity CVE must be replaced or the affected code path disabled within 7 days.

### 11.4 Vendoring

- Don't vendor third-party code. Use the package manager.
- Exception: tiny utility functions (under 50 lines) may be inlined with attribution.

---

## 12. Git workflow & commits

### 12.1 Branches

- `main` is always releasable. Direct pushes are forbidden.
- Feature branches: `feat/<short-description>`.
- Fix branches: `fix/<short-description>`.
- Refactor branches: `refactor/<short-description>`.
- Documentation: `docs/<short-description>`.

### 12.2 Commit messages

Format:
```
<type>: <imperative summary, under 72 chars>

<wrapped body explaining WHY, not WHAT. Wrap at 72 chars.>

<optional: trailing references like Refs #123 or Fixes #456>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `style`.

Examples:

```
feat: add Time Machine replay timeline

The hash chain made point-in-time replay possible architecturally,
but there was no UI to access it. This adds a scrubber that lets
users select any timestamp in a session and see both the frame
state and the events that fired in that window.

Refs #45
```

```
fix: prevent unwrap panic in stop_capture

If a session was stopped after being externally cleared (e.g. by
a config reload), the as_mut().unwrap() at commands.rs:142 would
panic. Replaced with ?-propagating ok_or_else returning a clear
error string the UI can surface.
```

Bad examples:
- `update stuff` ❌
- `fix bug` ❌
- `WIP` ❌

### 12.3 PR size

| Lines changed | OK? |
|---|---|
| < 200 | ✅ Ideal |
| 200–500 | ✅ Acceptable |
| 500–1,000 | ⚠️ Needs justification |
| > 1,000 | ❌ Must be split unless it's a generated file or a single mechanical refactor |

### 12.4 Mandatory CI checks before merge

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `npm run build` (verifies frontend builds)
- All review threads resolved

---

## 13. Code review

### 13.1 Reviewer responsibilities

Every PR needs at least one reviewer's approval. The reviewer's job:

1. **Verify it does what it claims.** Read the PR description, then read the diff. Do they match?
2. **Check the architectural invariants** in §2. Any violation is a blocker.
3. **Look for missing tests.** §7.3 specifies what's required for each change type.
4. **Look for breaking changes.** Public API, on-disk format, event schema — all need version bumps or deliberate handling.
5. **Run it locally** if the change is non-trivial. CI green is not sufficient for capture-engine changes.

### 13.2 Author responsibilities

1. **Self-review before requesting review.** Read your own diff line by line.
2. **PR description follows the template** (see `.github/pull_request_template.md` — to be added).
3. **Address all comments** before re-requesting review. Either fix, push back with reasoning, or file a follow-up issue and link it.
4. **Don't merge your own PRs** without explicit reviewer sign-off.

### 13.3 Review etiquette

- Comment on code, not on people.
- "This..." not "You..."
- Suggest a concrete alternative when criticizing.
- Use suggestions blocks for trivial diffs.
- Approve with comments is fine for minor nits; require changes for anything that violates this document.

---

## 14. Documentation

### 14.1 Required documentation

For every public type, function, and module in `traceflow_lib`:

```rust
/// One-line summary in active voice.
///
/// Longer explanation if needed. Cover:
///   - what it does (not how, unless surprising)
///   - what callers must guarantee (invariants on inputs)
///   - what guarantees this provides on outputs
///   - any side effects (writes to disk, sends events, mutates state)
///
/// # Examples
///
/// ```
/// // a minimal usage example, if non-obvious
/// ```
///
/// # Errors
///
/// Describes the conditions that cause an Err return.
pub fn append(&self, body: EventKind) -> Result<EventRecord> {
    ...
}
```

### 14.2 Module-level docs

Every module starts with a `//!` block explaining the module's purpose and how it fits into the architecture. See `events/chain.rs` for the standard.

### 14.3 Files to keep current

| File | When to update |
|---|---|
| `README.md` | Any user-facing behavior change |
| `USER_MANUAL.md` | New features, changed UI |
| `AGENTS.md` | Anything an AI agent needs to know about the codebase |
| `CLAUDE.md` | Same as AGENTS.md |
| `CHANGELOG.md` | Every PR (under "Unreleased") |
| `ENGINEERING_STANDARDS.md` | When the standards themselves change |
| Module-level `//!` docs | When the module's role changes |

### 14.4 Comment quality

- Comments explain *why*, not *what*. The code shows what. If the *what* needs a comment, the code needs to be clearer.
- TODOs include an owner and a tracking issue: `// TODO(@alice, #45): add OCR backpressure`.
- "Fix this later" comments without an issue are forbidden.

---

## 15. Tooling & automation

### 15.1 Required local tooling

| Tool | Version | Install |
|---|---|---|
| Rust | matches `rust-toolchain.toml` | `rustup install stable` |
| Node.js | 20 LTS or newer | `winget install OpenJS.NodeJS.LTS` |
| `rustfmt` | bundled with Rust | `rustup component add rustfmt` |
| `clippy` | bundled with Rust | `rustup component add clippy` |
| `cargo-audit` | latest | `cargo install cargo-audit` |
| `cargo-deny` (optional) | latest | `cargo install cargo-deny` |

### 15.2 Pre-commit hooks

A `.githooks/pre-commit` script (to be added) runs:
1. `cargo fmt --check`
2. `cargo clippy -- -D warnings` (changed files only)
3. `cargo test --lib` (fast tests only)

Enable with `git config core.hooksPath .githooks`.

### 15.3 CI pipeline

Per §12.4. Add new checks here when they prove valuable; don't add them just because you can.

### 15.4 Editor configuration

`.editorconfig` is checked in. VS Code workspace settings under `.vscode/settings.json` set up `rust-analyzer`, `eslint`, and `prettier` integration.

---

## 16. Working with Claude Code / agents

Traceflow is partially developed with AI assistance. This is fine, encouraged even. Apply these rules:

### 16.1 Agent rules

- `AGENTS.md` and `CLAUDE.md` must be current. They are the agent's primary context.
- Agents follow this document. PRs from agents are reviewed by the same standards as human PRs.
- Agents must never:
  - Disable a test instead of fixing it
  - `#[allow]` a lint without a `reason: ...` comment
  - Modify generated files (locks, lockfiles, build artifacts)
  - Change the event schema without a human reviewing it first

### 16.2 Prompting standards

When delegating to Claude Code or another agent:

- Give it the issue or task description, not "fix this."
- Ask for a plan before the code. Approve the plan, then ask for the code.
- For changes touching the hash chain, capture engine, or redaction pipeline: require the agent to write the tests first, get them reviewed, then write the implementation.

### 16.3 Acceptance criteria

Every agent-authored PR includes a section in the description:

```
## Agent context
- Prompt: <what was asked>
- Plan reviewed: <yes/no, by whom>
- Tests written first: <yes/no>
- Local build: <pass/fail>
- Local tests: <pass/fail>
```

If any of those are "no" or "fail," the PR needs a human to either complete the missing step or explicitly justify the exception.

---

## 17. Anti-patterns to avoid

A non-exhaustive catalog of things we've seen and don't want to see again:

### 17.1 The "in-memory shadow log"

Tempting: keep a `Vec<Step>` in `AppState` and update both it and the event log on every change.
Forbidden because: they will drift. The log is the truth (§2.3). Always re-project from the log after mutations.

### 17.2 The "trust the filename" pattern

Tempting: assume `frames/abc123.png` actually hashes to `abc123`.
Forbidden because: an attacker (or a bug) can put any bytes under any name. Verify content-addresses when you load them in any security-sensitive path.

### 17.3 The "swallow and continue" pattern

Tempting: `if let Err(e) = log.append(event) { log::warn!("..."); }` in the capture loop.
Forbidden because: a missing event silently breaks the chain. Capture must stop on log write failure.

### 17.4 The "I'll add it to config later" pattern

Tempting: hardcode a special case for one customer / industry / app.
Forbidden because: industry specifics live in rule packs and templates (§2.4). If you can't express it that way, propose extending the rule pack or template schema.

### 17.5 The "tests are slow, skip them" pattern

Tempting: `#[ignore]` a flaky test or `cargo test --skip`.
Forbidden because: the test fails for a reason. Either fix the test or fix the code. If you must defer, file an issue and reference it: `#[ignore = "see #123"]`.

### 17.6 The "while I'm here" PR

Tempting: a bug fix PR that also reformats five other files.
Forbidden because: reviewers can't tell signal from noise. Separate PRs for separate concerns.

### 17.7 The "JSON config in the UI" pattern

Tempting: expose a raw JSON editor for rule packs to "power users."
Forbidden because: the wizard and builder exist to abstract this. JSON is the storage format; the UI is the experience (per the conversation that produced this document).

---

## 18. Exceptions process

Any rule in this document can be broken **once**, in a specific PR, with a written exception.

### 18.1 What an exception requires

In the PR description, include a section:

```
## Standards exception
- Rule: <which rule from ENGINEERING_STANDARDS.md>
- Reason: <why this PR can't comply>
- Mitigations: <what we did to limit the blast radius>
- Cleanup plan: <when and how we restore compliance>
- Tracking issue: <link>
```

### 18.2 What a reviewer does

The reviewer approves the exception if and only if:
1. The cleanup plan is concrete and time-bound.
2. The tracking issue exists.
3. The exception is genuinely necessary, not a shortcut.

Exceptions are reported in the next standup. Repeated exceptions in the same area indicate the rule needs revisiting — propose an amendment to this document.

### 18.3 Amending this document

This document is versioned. Amendments are proposed via PR. Significant changes (anything in §2) require team consensus, not just a single reviewer's approval.

---

## Appendix A: Quick reference

### A.1 Pre-PR checklist

Before opening a PR, the author confirms:

- [ ] `cargo fmt` run
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] `npm run build` passes
- [ ] No new `unwrap()` / `expect()` in production code
- [ ] No new `any` in TypeScript
- [ ] No new dependencies added (or §11.1 questionnaire answered in the description)
- [ ] Architectural invariants (§2) preserved
- [ ] Doc comments on new `pub` items
- [ ] Tests added per §7.3
- [ ] `CHANGELOG.md` entry under "Unreleased"
- [ ] Commit messages follow §12.2

### A.2 Common Rust patterns

```rust
// IPC command shape
#[tauri::command]
pub async fn some_action(
    arg: ArgType,
    state: State<'_, SharedState>,
) -> Result<ReturnType, String> {
    let guard = state.lock().await;
    // delegate to a real function in a submodule
    submodule::do_real_work(&guard, arg).map_err(|e| e.to_string())
}

// Appending an event
let rec = log.append(EventKind::SomeEvent { ... })?;

// Error context
let bytes = std::fs::read(&path)
    .with_context(|| format!("reading rule pack {}", path.display()))?;

// Spawning blocking work
let result = tokio::task::spawn_blocking(move || {
    expensive_sync_function(input)
}).await??;  // double ? for JoinError + inner Result

// Conditional Windows code
#[cfg(target_os = "windows")]
fn windows_specific() -> Result<Foo> { ... }

#[cfg(not(target_os = "windows"))]
fn windows_specific() -> Result<Foo> {
    anyhow::bail!("this feature is currently Windows-only")
}
```

### A.3 Common TypeScript patterns

```ts
// IPC call with type safety
try {
  const result = await invoke<StepView[]>("get_session_steps");
  setSteps(result);
} catch (e) {
  // user-facing translation
  setError(translateIpcError(e));
}

// useEffect with cleanup
useEffect(() => {
  let un: UnlistenFn | null = null;
  listen("step-captured", handleStep).then((u) => (un = u));
  return () => {
    if (un) un();
  };
}, []);
```

---

## Appendix B: Glossary

- **Capture loop** — the background task that polls the screen and emits events
- **Chain** — the SHA-256 linked list of event records that gives Traceflow its tamper-evidence
- **Event log** — the append-only NDJSON file that records every observable thing in a session
- **Hot loop** — code that runs many times per second in the capture path
- **Hot path** — any code that runs while a capture session is active
- **IPC** — Tauri inter-process communication: JS frontend → Rust backend
- **Projection** — building a derived view (e.g. step list) from the event log
- **Promotion** — the moment a candidate frame passes the stability check and becomes a step
- **Rule pack** — a JSON file defining patterns to redact or flag
- **Source of truth** — the event log. Everything else is a cache or projection.

---

*End of document. Acknowledge by ticking "I have read and will follow ENGINEERING_STANDARDS.md" in the PR template.*
