# Traceflow

> Workflow capture & documentation platform.
> Tamper-evident event log → renderable into any format → customizable per industry by config, never by code.

## What it is

Traceflow watches you walk through any workflow — software installation, ERP configuration, lab procedure, customer support flow, anything happening on screen — and produces a tamper-evident record. That record can be projected into a polished Word document, a Markdown guide, an HTML page, an audit JSON, a training video, or any custom format you templatize.

The architecture rests on four ideas:

| Idea | Why it matters |
|---|---|
| Append-only event log | Single source of truth. Every capture is just a projection of the log. |
| SHA-256 hash chain | Tampering breaks the chain. Forensic-grade evidence. |
| Template-driven outputs | Layout, fonts, branding live in JSON / Jinja templates. No code changes. |
| Pluggable rule packs | Customers ship their own JSON for PII, secrets, compliance patterns. |

## The "everything is an event" model

Every observable thing during a capture session is appended to `events.ndjson`:

- `session_start`, `session_end`
- `step_promoted` (frame passed change detection)
- `ai_description`, `description_edited`
- `step_deleted`
- `window_focus_changed`, `mouse_click`, `keyboard_input` (phase 2)
- `ocr_result`, `redaction_applied`

Each record carries:
- `seq` — monotonic sequence number
- `prev` — SHA-256 of the previous record
- `hash` — SHA-256 of this record's canonical JSON
- `body` — the event payload

Tampering with any record breaks the chain. The product can prove it.

```
seq=0  prev=000...  hash=A  body=SessionStart{...}
seq=1  prev=A       hash=B  body=StepPromoted{...}
seq=2  prev=B       hash=C  body=AiDescription{...}
seq=3  prev=C       hash=D  body=SessionEnd{...}
```

## Architecture

```
                     ┌─────────────────────────────────┐
                     │   Capture Engine                │
                     │   (pixel + accessibility +      │
                     │    input hooks — phase 2)       │
                     └──────────────┬──────────────────┘
                                    │ events
                                    ▼
       ┌────────────────────────────────────────────────────────┐
       │   Append-only event log (NDJSON, SHA-256 chained)      │
       └────┬───────────────┬───────────────┬───────────────────┘
            │               │               │
            ▼               ▼               ▼
       ┌─────────┐    ┌──────────┐   ┌──────────────┐
       │ Document│    │ Rules    │   │ Knowledge    │
       │ Engine  │    │ Engine   │   │ Engine       │
       │         │    │ (JSON    │   │ (phase 3)    │
       │ Tmpl→   │    │  packs)  │   │              │
       │ .docx   │    │          │   │              │
       │ .md     │    │          │   │              │
       │ .html   │    │          │   │              │
       │ .json   │    │          │   │              │
       └─────────┘    └──────────┘   └──────────────┘
```

## Project layout

```
traceflow/
├── traceflow.config.toml              ← per-project customization
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── templates/                   ← shipped templates
│   │   ├── default.docx.json        ← .docx layout spec
│   │   ├── audit.docx.json          ← landscape audit layout
│   │   ├── default.md.j2            ← Markdown template (Jinja)
│   │   └── default.html.j2          ← HTML template (Jinja)
│   ├── rule_packs/                  ← shipped rule packs
│   │   ├── general_pii.json
│   │   ├── secrets.json
│   │   └── financial.json
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── state.rs                 ← thin state (log is the truth)
│       ├── commands.rs              ← Tauri IPC surface
│       ├── config.rs                ← traceflow.config.toml loader
│       ├── events/
│       │   ├── event.rs             ← Event enum & SessionPaths
│       │   ├── log.rs               ← append-only NDJSON writer
│       │   └── chain.rs             ← SHA-256 hash chain
│       ├── capture/
│       │   ├── engine.rs            ← emits events to log
│       │   ├── diff.rs              ← perceptual pixel diff
│       │   └── monitor.rs           ← multi-monitor
│       ├── document/
│       │   ├── render.rs            ← projection + dispatch
│       │   ├── docx_renderer.rs     ← .docx from JSON spec
│       │   └── text_renderer.rs     ← MiniJinja for .md/.html/.txt
│       ├── rules/
│       │   └── engine.rs            ← rule pack runtime
│       ├── privacy/
│       │   └── redact.rs            ← image-level blur/black-box primitives
│       └── ai/
│           └── describer.rs         ← step descriptions (local model swap-in)
└── src/                              ← React + TypeScript UI
    ├── App.tsx
    ├── components/
    │   ├── CaptureControl.tsx
    │   ├── StepGallery.tsx
    │   └── ExportPanel.tsx          ← multi-format + chain verify
    ├── main.tsx
    └── styles.css
```

## Customization without code

Every customer customization is a file, not a build:

| Want to change | Edit this file |
|---|---|
| Which PII patterns get redacted | `rule_packs/*.json` (or write your own) |
| Word doc layout, fonts, branding | `templates/*.docx.json` |
| Markdown / HTML output structure | `templates/*.j2` (Jinja) |
| Capture sensitivity, fps, language | `traceflow.config.toml` |
| Add a brand-new output format | Write a `.j2` template, point config at it |

## Built-in rule packs

- **general_pii** — emails, phone numbers, IPv4
- **secrets** — AWS keys, JWTs, GitHub/OpenAI tokens, PEM private keys
- **financial** — credit cards (Luhn-validated), US SSN, IBAN, SWIFT/BIC

Customers stack any combination. They can ship their own internal packs
identical in format to these.

## Phase roadmap

**Phase 1 (this code) — MVP horizontal product**
- Event log + hash chain ✓
- Cross-platform capture (pixel-based) ✓
- Windows-first OCR + input hooks — redaction and input capture work fully on Windows today; Linux/macOS support is planned.
- Template-driven .docx / .md / .html / .json export ✓
- Pluggable rule packs ✓
- Project config ✓

**Phase 2 — Workflow intelligence**
- OS accessibility hooks (UIA / AX / AT-SPI) as additional capture sources
- Input hooks (mouse / keyboard) for higher-signal events
- OCR pass + image-level redaction of detected patterns
- Local vision-language model for step descriptions (candle / llama.cpp)
- Version-diff mode: compare two sessions
- Time Machine Mode: deterministic replay of any session at any point

**Phase 3 — Knowledge & integrations**
- Full-text search across session logs
- Workflow graph: discover repeated patterns across sessions
- Plugin SDK in Rust for custom capture sources & exporters
- Optional cloud sync, SSO, audit retention policies

## Getting started

```bash
unzip traceflow-v2.zip
cd traceflow
npm install
npm run tauri dev
```

Requires the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) plus
Node 20+ and Rust 1.78+.

## Try a custom rule pack

Create `rule_packs/my_company.json`:

```json
{
  "name": "my_company",
  "version": "1.0.0",
  "rules": [
    {
      "name": "internal_ticket",
      "pattern": "\\bACME-\\d{4,}\\b",
      "action": { "type": "mask", "replacement": "[TICKET]" },
      "severity": "low"
    }
  ]
}
```

Then in `traceflow.config.toml`:

```toml
[rules]
packs = ["general_pii.json", "secrets.json", "my_company.json"]
```

No code change. No rebuild. That's the point of the platform.

## License

Proprietary. All rights reserved.
