# Traceflow User Manual

**Version 0.2 · Phase 1 & 2**

Traceflow captures your screen as you walk through any workflow and exports it as a polished, tamper-evident document. Everything happens offline — no cloud, no accounts.

---

## Table of Contents

1. [Getting Started](#1-getting-started)
2. [The Interface](#2-the-interface)
3. [Recording a Session](#3-recording-a-session)
4. [Capture Settings](#4-capture-settings)
5. [Working with Steps](#5-working-with-steps)
6. [Exporting Documents](#6-exporting-documents)
7. [Verifying Chain Integrity](#7-verifying-chain-integrity)
8. [Session History](#8-session-history)
9. [PII Redaction](#9-pii-redaction)
10. [Forensic Mode](#10-forensic-mode)
11. [Configuration File](#11-configuration-file)
12. [Rule Packs](#12-rule-packs)
13. [Troubleshooting](#13-troubleshooting)

---

## 1. Getting Started

### First launch

Run the app from the Start menu or your desktop shortcut. On first launch:

- A default configuration is created at `Documents\Traceflow\traceflow.config.toml`
- Session data is stored under `Documents\Traceflow\sessions\`
- No internet connection is required

### What Traceflow records

Each recording session produces an **append-only event log** (`events.ndjson`) with a SHA-256 hash chain. Every promoted screenshot, every window focus change, every mouse click, and every keyboard action is a chained event. The chain cannot be altered without detection — this is what makes exports forensically defensible.

---

## 2. The Interface

```
┌─────────────────────────────────────────────────────────┐
│  Traceflow   v0.2 · event-sourced · offline   session … │  ← Top bar
├──────────────┬──────────────────────────────────────────┤
│              │                                          │
│   SIDEBAR    │            STEP GALLERY                  │
│              │                                          │
│  • Capture   │  Step 1    Step 2    Step 3  …           │
│  • Settings  │  [thumb]   [thumb]   [thumb]             │
│              │                                          │
│  • History   │  Click a step to see it full-size        │
│              │  and edit its description                │
│  • Export    │                                          │
│  • Audit     │                                          │
├──────────────┴──────────────────────────────────────────┤
│  ● Recording · 8 fps · threshold 4.0%  PII on · AI on  │  ← Status bar
└─────────────────────────────────────────────────────────┘
```

**Top bar** — shows the current session ID (first 8 characters) so you can match it to the exported document.

**Sidebar** — contains all controls: capture settings, session history, export buttons, settings, and chain verification.

**Settings tab** — opens the rule pack manager and quick builder. This is where you can enable or disable any installed pack, refresh the pack list, or create a new custom pack without directly editing JSON.

**Step gallery** — shows every captured screenshot as a thumbnail. Click any step to view it full-size and edit its description.

**Status bar** — live readout of recording state, poll rate, sensitivity threshold, and whether PII redaction and AI descriptions are active.

---

## 3. Recording a Session

### Starting a recording

1. Type a **title** for your session in the "Title" field (e.g. "Installing Acme Pro 5.2").  
   The title appears in all exported documents, so make it descriptive.
2. Select which **monitor** to capture if you have multiple screens.
3. Adjust any **capture settings** (see Section 4) if needed.
4. Click **● Start recording**.

The status bar turns red and shows "Recording". The app begins polling your screen in the background. You can switch to any other application — Traceflow continues capturing.

### During recording

- Switch to the applications you want to document and work through your workflow normally.
- Traceflow detects meaningful screen changes automatically. You do **not** need to click anything in Traceflow while recording.
- Each detected change appears as a new thumbnail in the step gallery within a second or two.
- Window title, app name, and (if enabled) OCR text are recorded with each step automatically.

### Stopping a recording

Click **■ Stop recording** in the sidebar. The session is finalised — a `SessionEnd` event is written to the log. You can now export, verify, and edit steps without restriction.

> **Tip:** You do not need to re-open a session to export it. The app retains the most recent session in memory until you start a new one.

---

## 4. Capture Settings

All settings take effect from the **next** recording session. They are persisted to `traceflow.config.toml` when you click Start.

| Setting | Default | What it does |
|---|---|---|
| **Title** | "Untitled documentation" | Name of the session in all exports |
| **Monitor** | Primary | Which screen to capture |
| **Poll fps** | 8 | How many times per second the screen is sampled. Higher = catches faster changes, uses more CPU. |
| **Sensitivity (threshold)** | 4.0 % | How different a frame must be from the previous one to be promoted as a step. Lower = more captures; higher = only big changes. |
| **Language** | English | Language hint for OCR and AI descriptions |
| **AI step descriptions** | On | Automatically generates a heading for each step from the window title and app name |
| **Auto-redact PII & passwords** | On | Runs OCR on each frame and blurs/masks any text matching the active rule packs |

### Rule pack manager
Open the **Settings** tab to review installed rule packs and manage which packs are active for new recording sessions.

- Installed packs appear with an enabled/disabled state.
- Click **Enable** to activate a pack for future recordings.
- Click **Disable** to deactivate a pack without deleting the file.
- Click **Refresh** after adding rule pack JSON files externally so Traceflow re-scans the available packs.

### Quick rule builder
The Settings tab also includes a wizard for creating a custom rule pack.

- Enter a pack name, version, and description.
- Define a single rule with a name, regex pattern, action, and severity.
- Click **Save rule pack** to write it into your user pack directory.
- The new pack is automatically added to the active pack list when saved.

### Sensitivity guide

| Threshold | Best for |
|---|---|
| 1–2 % | Wizard steps, installers, small dialog changes |
| 4 % (default) | Most workflows |
| 8–15 % | Presentations, major screen transitions only |

### Minimum dwell time

Traceflow requires a screen to be **stable for at least one poll interval** before promoting it as a step. At 8 fps this is ~125 ms. If you click through screens faster than that, some may be missed. The promote-on-replace mechanism ensures that even rapidly-navigated screens are saved as long as they appear in at least one poll frame.

---

## 5. Working with Steps

### Viewing a step

Click any thumbnail in the gallery to see the full-size screenshot alongside its metadata:

- **Heading** — auto-generated from the window title (e.g. "GitHub — Microsoft Edge", "VS Code — main.rs", "Excel — budget.xlsx")
- **Window title** — the exact title bar text at capture time
- **Application** — the executable name of the focused window
- **Captured at** — UTC timestamp

### Editing a description

Click the description text below any step to edit it inline. Press Enter or click away to save. Your edit is appended to the event log as a `DescriptionEdited` event — the original AI-generated text is preserved in the chain.

User-edited descriptions always win over AI-generated ones in exports.

### Deleting a step

Click the **×** button on a thumbnail to delete a step. The deletion is recorded as a `StepDeleted` event — the original frame file and event remain in the log (the chain is never altered), but the step is excluded from all exports and re-indexing happens automatically.

You can delete steps at any time, including after the session has ended.

---

## 6. Exporting Documents

Click any export button in the **Export** section of the sidebar. A save-file dialog opens. Every export is a fresh **projection** of the event log — deleted steps are excluded and edited descriptions are applied.

### Export formats

| Button | Output | Best for |
|---|---|---|
| **↓ Export .docx (Word)** | Microsoft Word document with embedded images | Formal reports, client deliverables, compliance evidence |
| **↓ Export .md (Markdown)** | Plain text with image links | Developer docs, wikis, version-controlled documentation |
| **↓ Export .html (Web)** | Styled HTML page | Sharing via browser, internal portals |
| **↓ Export .json (Audit)** | Full structured data including metadata | Programmatic processing, audit trail integration |

### What's in each export

All formats include:

- Session title, author, and generation timestamp
- Each step in sequence (re-indexed after deletions)
- Step description (user-edited or AI-generated)
- Window title at capture time
- Embedded or linked screenshot
- Session ID and event count for audit traceability

### .docx file size

Screenshots are re-encoded as JPEG (quality 82) before embedding, keeping a typical 26-step document to 1–3 MB rather than 15+ MB with raw PNG.

---

## 7. Verifying Chain Integrity

Click **✓ Verify chain integrity** in the Audit section.

Traceflow re-reads the event log from disk and recomputes the SHA-256 hash chain from the very first event. The result is shown as a toast notification:

- **chain intact — N events verified** — the log has not been altered since recording
- **chain broken at seq=N** — a record was modified, deleted, or the file was edited externally

The chain covers every event: `SessionStart`, `StepPromoted`, `OcrResult`, `RedactionApplied`, `AiDescription`, `DescriptionEdited`, `StepDeleted`, `WindowFocusChanged`, `MouseClick`, `KeyboardInput`, `SessionEnd`. Any external modification to `events.ndjson` will fail verification.

> **For legal/compliance use:** export the `.json` format — it contains the full chain (prev hash and hash fields) for each event, which can be independently re-verified by any SHA-256 implementation.

---

## 8. Session History

The **Past sessions** button in the sidebar lists all sessions recorded on this machine, most-recent first. Each entry shows:

- Session title
- Date and time recorded
- Number of steps captured

### Loading a past session

Click any entry in the history list. The session is loaded into memory — its steps appear in the gallery and all export/verify/edit operations work against it, exactly as if you had just stopped recording it.

> **Note:** Loading a past session replaces the current in-memory session. Your current steps panel will be updated. The loaded session's event log on disk is unchanged — any edits or deletions you make are appended as new events.

---

## 9. PII Redaction

When **Auto-redact PII & passwords** is enabled, Traceflow runs Windows OCR (built-in, offline) on each promoted frame immediately before saving it to disk. The OCR output is evaluated against the active rule packs.

### How redaction works

1. Frame is captured and passed to OCR.
2. Each recognised word is matched against regex rules in the active packs.
3. For words that match a `blur` or `black_box` rule, the pixel region is blurred or blacked out on the frame **before** the PNG is hashed and saved.
4. The saved screenshot is already redacted — the original unredacted pixels are never written to disk.
5. A `RedactionApplied` event is appended to the chain, recording which rule triggered and how many matches were found (but not the matched text itself).

### What gets redacted by default

The bundled rule packs (`general_pii.json`, `secrets.json`, `financial.json`) cover:

| Pack | Detects |
|---|---|
| `general_pii` | Email addresses, US phone numbers, IPv4 addresses |
| `secrets` | API keys, tokens, private key headers, connection strings |
| `financial` | Credit card numbers (Luhn-validated), IBAN, routing numbers |

### Turning redaction off

Uncheck **Auto-redact PII & passwords** before starting a recording. The OCR and rules engine are not invoked; full unredacted screenshots are saved.

---

## 10. Forensic Mode (keep_all_frames)

When **keep_all_frames** is enabled in the capture settings (or `traceflow.config.toml`), every sampled frame that differs from the reference — even frames that fall below the promotion threshold — is written to disk and recorded as a `FrameSampled` event.

**Use this when:**
- You need a continuous, unbroken visual record of every second (e.g. legal evidence, incident replay)
- You want to reconstruct activity between promoted steps

**Trade-off:** Disk usage scales with `poll_fps`. At 8 fps over a 10-minute session, expect 10–30 GB of PNG files. Reduce fps or use JPEG compression if space is a concern.

---

## 11. Configuration File

Located at `Documents\Traceflow\traceflow.config.toml`. Created automatically on first launch with defaults. Edit with any text editor; changes take effect on the next session start.

```toml
[project]
name = "My Documentation Project"
author = "Your Name"

[capture]
poll_fps = 8                  # 2 | 4 | 6 | 8
change_threshold = 0.04       # 0.01–0.20
stability_frames = 1          # frames of stability required before promoting
ai_describe = true
redact_pii = true
language = "en"               # en | es | fr | de | ja | zh
keep_all_frames = false
monitor_index = 0             # 0 = primary monitor

[rules]
packs = ["general_pii.json", "secrets.json"]

[templates]
docx_spec = "default.docx.json"
markdown = "default.md.j2"
html = "default.html.j2"

[branding]
accent_color = "#c33c1f"
footer = "Generated with Traceflow"
```

---

## 12. Rule Packs

Rule packs are JSON files that define what counts as sensitive information. They live in the `rule_packs/` folder next to the Traceflow executable (or `src-tauri/rule_packs/` in development).

### Writing a custom rule

```json
{
  "name": "my_company",
  "version": "1.0.0",
  "description": "Internal sensitive patterns",
  "rules": [
    {
      "name": "employee_id",
      "pattern": "\\bEMP-\\d{6}\\b",
      "action": { "type": "mask", "replacement": "[EMP-ID]" },
      "severity": "medium",
      "description": "Internal employee ID format"
    },
    {
      "name": "project_code",
      "pattern": "\\bPRJ-[A-Z]{3}-\\d{4}\\b",
      "action": { "type": "blur" },
      "severity": "high"
    }
  ]
}
```

### Action types

| Action | Effect |
|---|---|
| `{ "type": "blur" }` | Gaussian blur applied to the pixel region |
| `{ "type": "black_box" }` | Solid black rectangle over the region |
| `{ "type": "mask", "replacement": "[TEXT]" }` | Text-level replacement (recorded in log, no pixel change) |
| `{ "type": "flag" }` | No redaction — only records a `RedactionApplied` event |
| `{ "type": "drop" }` | Excludes the event entirely from the log (use with caution) |

### Adding your pack

1. Save your JSON file to the `rule_packs/` directory.
2. Add the filename to `traceflow.config.toml`:
   ```toml
   [rules]
   packs = ["general_pii.json", "secrets.json", "my_company.json"]
   ```
3. Restart a recording session — packs are loaded at session start.

### In-app pack management
You can also manage rule packs directly in Traceflow's **Settings** tab.

- Installed packs are listed with an enabled/disabled state.
- Use the **Enable** / **Disable** buttons to change activation without editing config files.
- Use **Refresh** after adding or removing JSON files from the rule pack directories.
- Use the quick builder to save a custom rule pack to your user pack directory.

---

## 13. Troubleshooting

### Steps are not appearing / too few captured

- **Threshold too high:** Lower the Sensitivity slider. Try 2–3 % for wizard-style workflows.
- **Navigating too fast:** Stay on each screen for at least 125 ms (one poll tick at 8 fps). The status bar shows the current rate.
- **Dark-themed app:** The hybrid diff algorithm (MAD + changed-pixel count) is specifically tuned for dark UIs. If still missing steps, lower the threshold to 2 %.

### Too many duplicate / noisy steps

- **Threshold too low:** Raise Sensitivity. 6–8 % is a good starting point for video-heavy or animated UIs.
- **Clock or animated element:** The stability window (1 confirmation frame) filters most animated content. If a specific app keeps triggering, raise stability_frames to 2 in `traceflow.config.toml`.

### OCR / redaction is slow

- OCR runs on every promoted frame using the Windows built-in engine. On older hardware, this can add 0.5–2 s per step.
- If speed matters more than redaction, uncheck **Auto-redact PII & passwords**.
- OCR quality depends on screen resolution and font size. Increase display scaling if text isn't being detected.

### Export produces an empty or short document

- Check that steps are visible in the gallery — if you deleted all steps, the export will be empty.
- Make sure you are exporting **after stopping** the recording (not while it is active); active-session exports work too, but only include steps captured so far.

### Chain verification fails

- The `events.ndjson` file has been modified, truncated, or corrupted outside of Traceflow.
- The file was moved or copied to a different path (verification reads the log at the stored path).
- A sync tool (OneDrive, Dropbox) modified the file during a recording.
- **Resolution:** The export from the previous clean state remains valid. Start a new session for future recordings.

### "No past sessions found" in history panel

- Sessions are stored in `Documents\Traceflow\sessions\`. If this folder was moved or the drive letter changed, the history panel will be empty.
- Click "Past sessions" again — it refreshes each time the panel is opened.

### App crashes on start

- Ensure Windows 10 (build 10240) or later — the Windows Media OCR engine requires this minimum version.
- Check `Documents\Traceflow\` for a `traceflow.config.toml` with syntax errors. Delete the file to reset to defaults.

---

## Keyboard shortcuts

| Key | Action |
|---|---|
| None yet | All controls are mouse-driven in v0.2 |

Keyboard shortcuts are planned for Phase 3.

---

## Data locations

| Item | Path |
|---|---|
| Sessions | `Documents\Traceflow\sessions\{uuid}\` |
| Event log | `Documents\Traceflow\sessions\{uuid}\events.ndjson` |
| Frame PNGs | `Documents\Traceflow\sessions\{uuid}\frames\` |
| Configuration | `Documents\Traceflow\traceflow.config.toml` |
| Rule packs | `{app directory}\rule_packs\` |
| Templates | `{app directory}\templates\` |

All data is local. Nothing is transmitted over the network.

---

*Traceflow v0.2 · Generated with Traceflow*
