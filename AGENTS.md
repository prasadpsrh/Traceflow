# AGENTS.md — Agent onboarding for Traceflow

Purpose
- Provide minimal, actionable instructions for AI coding agents to be productive in this repository.

Quick start (commands)
- Frontend (Vite):
  - `npm install`
  - `npm run dev` — start Vite dev server
  - `npm run build` — build frontend to `dist/`
  - `npm run preview` — preview a production build
- Full stack (Tauri + frontend):
  - `npm run tauri dev` — runs Vite then launches the Tauri app for end-to-end dev
  - `npm run tauri build` — bundle production app (creates Rust release artifacts)

Important files and entry points
- Frontend entry: [src/main.tsx](src/main.tsx)
- UI root: [src/App.tsx](src/App.tsx)
- Frontend components: [src/components/](src/components/)
- Tauri entry & glue: [src-tauri/src/main.rs](src-tauri/src/main.rs) and [src-tauri/src/lib.rs](src-tauri/src/lib.rs)
- IPC commands: [src-tauri/src/commands.rs](src-tauri/src/commands.rs)
- Config loader: [src-tauri/src/config.rs](src-tauri/src/config.rs)
- Event model & logging: [src-tauri/src/events/](src-tauri/src/events/)
- Templates & rule packs: [src-tauri/templates/](src-tauri/templates/) and [src-tauri/rule_packs/](src-tauri/rule_packs/)
- Project config: [traceflow.config.toml](traceflow.config.toml), [tsconfig.json](tsconfig.json), [vite.config.ts](vite.config.ts), [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)

Agent conventions and guardrails
- Prefer `npm run tauri dev` for full-stack work (it launches Vite at `127.0.0.1:1420`).
- Do not modify `traceflow.config.toml` without noting the intended runtime impact (capture rates, rule packs).
- Respect TypeScript and Rust strictness: TypeScript `strict:true` and Rust toolchain pinned in [src-tauri/Cargo.toml](src-tauri/Cargo.toml).
- When changing IPC commands, update both `src-tauri/src/commands.rs` and the frontend callers in `src/`.
- There are no test scripts detected — avoid assuming test infra beyond build commands.

Helpful links for deeper context
- README and architecture notes: [README.md](README.md)
- Cargo manifest: [src-tauri/Cargo.toml](src-tauri/Cargo.toml)

If you want further automation
- I can also create `.github/copilot-instructions.md` or repository-specific skills (e.g., `skill:tauri-dev`, `skill:document-export`) that expose small run/debug recipes and common pitfalls.
