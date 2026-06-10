# Traceflow User Manual

> **Version:** 0.2.0
> **Covers:** Sprint 1 (Rule wizard, custom rules, pack management) and Sprint 2 (OCR, PII redaction, Time Machine replay)
> **Platform:** Windows (primary), macOS and Linux (partial — see §9)
> **Last updated:** June 2026

---

## Table of contents

1. [What is Traceflow](#1-what-is-traceflow)
2. [Installation and first launch](#2-installation-and-first-launch)
3. [Capturing a workflow](#3-capturing-a-workflow)
4. [Exporting documentation](#4-exporting-documentation)
5. [Redaction rules and PII protection](#5-redaction-rules-and-pii-protection)
6. [Time Machine replay](#6-time-machine-replay)
7. [Session management](#7-session-management)
8. [Configuration reference](#8-configuration-reference)
9. [Platform support matrix](#9-platform-support-matrix)
10. [Use cases](#10-use-cases)
11. [Troubleshooting](#11-troubleshooting)
12. [Glossary](#12-glossary)

---

## 1. What is Traceflow

Traceflow is a desktop application that **automatically captures meaningful screen changes** during any workflow — software installation, ERP configuration, system administration, quality assurance procedures, or any sequence of actions on your computer — and produces **polished documentation** from those captures.

### What makes it different

| Feature | What it means for you |
|---|---|
| **Smart change detection** | You don't press a button for each screenshot. Traceflow watches your screen and captures only when something meaningful changes. |
| **Tamper-evident audit trail** | Every capture session produces a cryptographically chained event log. If anyone modifies the record after the fact, the chain breaks and verification fails. |
| **Automatic PII redaction** | Emails, credit cards, API keys, and other sensitive data are detected and blurred or masked before screenshots are saved — configurable via rule packs. |
| **Multiple export formats** | One capture session can produce Word documents, Markdown, HTML, or JSON audit files — all from the same data. |
| **100% offline** | Your screen content never leaves your machine. No cloud, no telemetry, no account required. |

### How it works (30-second overview)

```
You work normally on your computer
        │
        ▼
Traceflow watches for screen changes (4× per second)
        │
        ▼
When the screen changes meaningfully and stabilizes,
a screenshot is captured automatically
        │
        ▼
OCR scans the screenshot for sensitive data
        │
        ▼
Rule packs identify PII (emails, cards, keys)
and redact it from the saved image
        │
        ▼
The redacted screenshot + metadata is appended to
a tamper-evident event log (SHA-256 hash chain)
        │
        ▼
You stop recording when done
        │
        ▼
Export to Word, Markdown, HTML, or JSON
```

---

## 2. Installation and first launch

### System requirements

| Component | Minimum | Recommended |
|---|---|---|
| OS | Windows 10 (build 1809+) | Windows 11 |
| RAM | 4 GB | 8 GB |
| Disk | 500 MB for installation | 2 GB+ for session storage |
| Display | 1280×720 | 1920×1080 or higher |
| WebView2 | Required (preinstalled on Win 11) | — |

### Installation

1. Download the Traceflow installer (`.msi` or `.exe`) for your platform.
2. Run the installer. Accept the defaults unless your IT policy requires a custom install path.
3. Launch Traceflow from the Start menu or desktop shortcut.

### First launch

On first launch, Traceflow:
- Creates a data directory at `C:\Users\<you>\Documents\Traceflow\`
- Generates a default `traceflow.config.toml` configuration file
- Loads the built-in rule packs (`general_pii` and `secrets`)
- Displays the main window with the Capture tab active

No account creation, no sign-in, no internet connection required.

### The main window

```
┌──────────────────────────────────────────────────────────┐
│  Traceflow                           session: none       │
│  v0.2 · event-sourced · offline      ⚙ Settings         │
├──────────┬───────────────────────────────────────────────┤
│          │                                               │
│ SIDEBAR  │  WORKSPACE                                    │
│          │                                               │
│ Capture  │  Captured steps appear here after recording   │
│ controls │                                               │
│          │                                               │
│ Export   │                                               │
│ options  │                                               │
│          │                                               │
├──────────┴───────────────────────────────────────────────┤
│  Idle · 4 fps · threshold 4.0%     PII rules on · 0 steps│
└──────────────────────────────────────────────────────────┘
```

The interface has three main areas:
- **Sidebar (left):** Capture controls (start/stop, sensitivity), export buttons, past sessions
- **Workspace (center/right):** Shows captured step thumbnails during and after recording
- **Status bar (bottom):** Current state, capture settings, step count

---

## 3. Capturing a workflow

### Starting a capture

1. Enter a **session title** in the sidebar (e.g., "Installing Acme Pro 5.2" or "Configuring SAP GL Accounts").
2. Optionally adjust capture settings:
   - **Poll FPS:** How often Traceflow samples the screen (2, 4, or 8 times per second). Default: 4.
   - **Sensitivity:** How much the screen must change to trigger a capture. Lower = more captures, higher = only major changes. Default: 4%.
   - **AI descriptions:** Toggle on/off. When on, each step gets an auto-generated description.
   - **PII redaction:** Toggle on/off. When on, detected sensitive data is blurred before saving.
3. Click **● Start recording**.

### What happens during recording

- The status bar shows **Recording** with a pulsing red dot.
- Traceflow monitors your screen in the background. **You don't need to keep the Traceflow window visible** — minimize it and work normally.
- Each time the screen changes meaningfully (a new dialog appears, a page loads, a form is submitted), Traceflow:
  1. Waits for the screen to **stabilize** (stops changing for a moment) to avoid capturing mid-transition.
  2. Takes a full-resolution screenshot.
  3. Runs OCR to extract text.
  4. Checks extracted text against active rule packs for PII.
  5. Redacts any matches (blur or black box).
  6. Saves the redacted screenshot with a SHA-256 content hash.
  7. Appends a `StepPromoted` event to the session log.
  8. Generates an AI description of what changed.
- Captured steps appear as thumbnail cards in the workspace in real time.

### During recording, you can

- **Switch between applications** freely — Traceflow captures whatever is on screen.
- **Use multiple monitors** — the configured monitor is captured (default: primary).
- **Minimize Traceflow** — it runs in the background.
- **Edit step descriptions** — click on any step's description text to modify it.
- **Delete steps** — click the ✕ button on a step card to remove it.

### Stopping a capture

Click **■ Stop recording**. The session remains loaded for export and review. You can:
- Export to any format
- Edit descriptions
- Delete steps
- Verify the chain integrity
- Open the Time Machine replay

### Capture tips

| Scenario | Recommended settings |
|---|---|
| Software installation (wizard-style) | FPS: 4, Sensitivity: 4% (default) |
| Web application walkthrough | FPS: 4, Sensitivity: 6–8% (web apps have subtle animations) |
| Terminal / CLI procedures | FPS: 2, Sensitivity: 2% (text changes are small) |
| Fast demo recording | FPS: 8, Sensitivity: 3% (capture everything) |
| Long-running process (overnight) | FPS: 2, Sensitivity: 8% (reduce disk usage) |

---

## 4. Exporting documentation

After stopping a capture (or loading a past session), the export buttons become active.

### Available formats

| Format | Best for | What you get |
|---|---|---|
| **Word (.docx)** | Sharing with non-technical stakeholders, printing, formal documentation | Title page + one section per step with embedded screenshot and description |
| **Markdown (.md)** | Developer documentation, wikis, GitHub READMEs | Clean Markdown with image references |
| **HTML (.html)** | Web publishing, intranet, email embedding | Self-contained HTML page with embedded images and print-friendly CSS |
| **JSON (.json)** | Audit evidence, API integration, programmatic processing | Full session data including event log metadata, step details, and chain hashes |

### How to export

1. Click the **↓ Export** button for your desired format.
2. A "Save as" dialog opens. Choose the destination and filename.
3. Traceflow renders the document from the session's event log and saves it.
4. A confirmation message shows the saved path.

### Customizing export appearance

Exports are driven by **templates** — JSON specs for Word, Jinja templates for Markdown/HTML.

**Built-in templates:**
- `default.docx.json` — Portrait layout, title page, full metadata
- `audit.docx.json` — Landscape layout, denser, timestamps on every step
- `default.md.j2` — Standard Markdown with image links
- `default.html.j2` — Styled HTML with print media query

**Custom templates:** Drop your own `.docx.json` or `.j2` files into `~/Documents/Traceflow/templates/` and they become available automatically.

### Export tips

- **For compliance/audit:** Use JSON export — it includes the full hash chain so auditors can verify no steps were added, removed, or modified after capture.
- **For training materials:** Use Word export — easy to add your own text, print, or convert to PDF.
- **For developer docs:** Use Markdown export — paste directly into GitHub, Confluence, or Notion.
- **For web publishing:** Use HTML export — self-contained, opens in any browser.

---

## 5. Redaction rules and PII protection

Traceflow can automatically detect and redact sensitive information from captured screenshots **before they are saved to disk**. This means the redacted version is the only version that exists — the original unredacted pixels are never persisted.

### How redaction works

```
Screenshot captured
       │
       ▼
OCR extracts text + bounding boxes from the image
       │
       ▼
Rule engine evaluates each word against active rule packs
       │
       ▼
Matches found? → Apply redaction action (blur / black box / mask)
       │
       ▼
Redacted image saved to disk (content-addressed by SHA-256)
       │
       ▼
Event log records: which rules matched, how many hits, what action was taken
(the actual matched text is NEVER saved — only its SHA-256 hash)
```

### Enabling/disabling redaction

Toggle **PII redaction** in the capture settings (sidebar). When enabled, the status bar shows "PII rules on."

### Built-in rule packs

Traceflow ships with three rule packs. Enable or disable them in **Settings → Redaction rules**:

| Pack | What it detects | Default |
|---|---|---|
| **general_pii** | Email addresses, US phone numbers, IPv4 addresses | ✅ Enabled |
| **secrets** | AWS access keys, JWTs, GitHub tokens, OpenAI keys, PEM private key headers | ✅ Enabled |
| **financial** | Credit card numbers (Luhn-validated), US SSNs, IBANs, SWIFT/BIC codes | Disabled |

### Using the rule wizard

The wizard provides 8 ready-to-use patterns for common PII types. No regex knowledge required.

1. Open **Settings** (top-right button).
2. Scroll to **Your custom rules**.
3. Click **✨ Add with wizard**.
4. Browse the preset cards organized by category:

| Category | Available presets |
|---|---|
| **Personal data** | Email addresses, Phone numbers (US) |
| **Financial** | Credit card numbers, US Social Security Numbers |
| **Secrets** | AWS access keys, JSON Web Tokens |
| **Network** | IP addresses (IPv4) |
| **Custom** | Internal ticket / ID prefix (e.g., ACME-1234) |

5. Click a card to select it. The rule builder opens pre-filled with the preset's pattern.
6. Optionally customize:
   - Change the **rule name**
   - Adjust the **replacement text** (e.g., `[EMAIL]`, `[REDACTED]`, `[CARD]`)
   - Change the **action** (blur, black box, mask, flag only)
   - Change the **severity** level
7. Click **Add rule**. The rule is saved and active immediately.

### Using the advanced rule builder

For patterns not covered by the wizard:

1. Open **Settings** → click **⚙ Advanced builder**.
2. Fill in:
   - **Rule name:** A short identifier (e.g., `employee_id`)
   - **Regex pattern:** The pattern to match (e.g., `\bEMP-\d{6}\b`)
   - **Test against sample text:** Paste example text to verify your pattern works. Matched substrings appear as highlighted chips in real time.
   - **Action:** What to do when matched (Mask, Black box, Blur, Drop, Flag)
   - **Replacement text:** (for Mask action) What to replace matches with
   - **Severity:** Low / Medium / High / Critical
   - **Validator:** Optional Luhn checksum for credit card validation
3. Click **Add rule**.

### The live regex tester

The most powerful feature of the rule builder. As you type a pattern:

- A **green ✓** appears if the pattern is valid regex
- A **red ⚠** appears with an error message if the pattern is invalid
- If you've entered sample text, **match chips** appear showing exactly what would be matched
- The match count updates in real time

**Example:**
```
Pattern:  \b[A-Z]{3}-\d{4}\b
Sample:   Order ABC-1234 received from XYZ-9999
Result:   ✓ Valid pattern · 2 matches in sample
          "ABC-1234"  "XYZ-9999"
```

### Managing rules

- **View custom rules:** Settings → Your custom rules section
- **Remove a rule:** Click **Remove** next to the rule name
- **Enable/disable a pack:** Toggle the checkbox next to any pack in Settings
- **Import a pack:** Click **↥ Import pack from file** and select a `.json` file
- **Export a pack:** Click **Export** next to any pack to save it as a shareable `.json` file

### Sharing rule packs with your team

1. Create your rules using the wizard or advanced builder.
2. Export your custom rules pack (Settings → Export on the `custom_rules` pack).
3. Send the `.json` file to colleagues.
4. They click **↥ Import pack from file** in their Traceflow settings.
5. The rules are immediately active in their installation.

No code changes, no configuration files to edit, no IT deployment needed.

### Privacy guarantees

- **OCR text is never saved to disk.** Only the SHA-256 hash of each detected word is recorded in the event log.
- **Redaction happens before the screenshot is saved.** The unredacted pixels never exist as a file on disk.
- **The redacted image's hash is what enters the chain.** The audit trail attests to the redacted version, not the original.
- **No data leaves your machine.** OCR runs locally (Windows.Media.Ocr on Windows), no cloud API calls.

---

## 6. Time Machine replay

Time Machine lets you **replay a captured session step by step**, seeing exactly what was on screen at each point along with the events that occurred nearby.

### Opening the replay viewer

1. After a capture session (or after loading a past session), click the **Time Machine** button in the workspace area.
2. The replay viewer opens, showing:
   - **Timeline scrubber** — a slider spanning the entire session duration
   - **Frame viewer** — the screenshot at the selected point in time
   - **Event panel** — events that occurred within ±500ms of the selected step

### Navigating the timeline

- **Drag the scrubber** left/right to move through the session.
- **Click Previous / Next** buttons to step through one capture at a time.
- **Timestamps** are shown at both ends of the scrubber and at the current position.
- **Step number** and window title are displayed above the frame.

### The event panel

For each step, the event panel shows everything that happened nearby:

| Event type | What it tells you |
|---|---|
| `step_promoted` | A screen change was detected and captured |
| `ai_description` | An AI-generated description was produced |
| `ocr_result` | Text was extracted from the screenshot |
| `redaction_applied` | PII was detected and redacted (shows rule name and match count) |
| `description_edited` | A user manually edited a step description |
| `session_start` / `session_end` | Session lifecycle events |

### Verifying chain integrity

From the replay viewer (or the export panel), click **✓ Verify chain integrity**. Traceflow scans the entire event log and verifies that:

1. Every event's sequence number is consecutive (no gaps).
2. Every event's `prev` hash matches the previous event's `hash`.
3. Every event's `hash` matches the SHA-256 of its canonical content.

If verification **passes**, you see a green message: *"chain intact — N events verified"*.

If verification **fails**, you see a red message indicating which event broke the chain. This means the log was modified after capture — a serious integrity issue.

### Use cases for Time Machine

- **Audit review:** An auditor can scrub through the exact sequence of screens seen during a procedure, with cryptographic proof that the record hasn't been modified.
- **Training replay:** A new team member can walk through a captured procedure step by step, seeing exactly what the original operator saw.
- **Incident investigation:** After a misconfiguration, replay the session to identify exactly which step introduced the error.
- **Compliance evidence:** Present the full session replay with chain verification as evidence that a procedure was followed correctly.

---

## 7. Session management

### Where sessions are stored

```
C:\Users\<you>\Documents\Traceflow\
├── traceflow.config.toml              ← global configuration
├── rule_packs\
│   └── custom_rules.json              ← your custom rules
└── sessions\
    └── <session-uuid>\
        ├── events.ndjson              ← the event log (source of truth)
        └── frames\
            └── <sha256>.png           ← content-addressed screenshots
```

Each session is a self-contained folder. You can:
- **Copy** a session folder to archive it
- **Move** it to a shared drive for team access
- **Zip** it for email or upload
- **Delete** the folder to remove the session permanently

### Viewing past sessions

Click **▼ Past sessions** in the sidebar to see previously captured sessions. Each shows:
- Session title
- Date and time
- Number of steps

Click a session to load it for viewing, exporting, or replaying.

### Session data integrity

Every session's event log (`events.ndjson`) is a tamper-evident record:

- Each line is one JSON event.
- Each event includes `seq` (sequence number), `prev` (hash of previous event), and `hash` (hash of this event).
- Modifying, inserting, or deleting any event breaks the hash chain.
- Use **✓ Verify chain integrity** to check at any time.

---

## 8. Configuration reference

### traceflow.config.toml

The global configuration file lives at `~/Documents/Traceflow/traceflow.config.toml`. You can edit it with any text editor, or use the Settings panel in the app.

```toml
[project]
name   = "My Documentation Project"
author = "Your Name"

[capture]
poll_fps          = 4       # Screen samples per second (2, 4, or 8)
change_threshold  = 0.04    # 0.0–1.0; lower = more captures
stability_frames  = 2       # Stable frames required before capturing
ai_describe       = true    # Auto-generate step descriptions
redact_pii        = true    # Run OCR + rule engine on captured frames
language          = "en"    # OCR language
monitor_index     = 0       # 0 = primary monitor
keep_all_frames   = false   # Forensic mode: save every sampled frame

[rules]
packs = [
    "general_pii.json",
    "secrets.json",
    # "financial.json",
]

[templates]
docx_spec = "default.docx.json"
markdown  = "default.md.j2"
html      = "default.html.j2"

[branding]
accent_color = "#c33c1f"
footer       = "Generated with Traceflow"
# logo_path  = "C:\\path\\to\\your\\logo.png"
```

### Rule pack JSON format

Custom rule packs follow this structure:

```json
{
  "name": "my_company_rules",
  "version": "1.0.0",
  "description": "Company-specific PII patterns",
  "rules": [
    {
      "name": "employee_id",
      "pattern": "\\bEMP-\\d{6}\\b",
      "action": { "type": "mask", "replacement": "[EMPLOYEE]" },
      "description": "Employee ID numbers",
      "severity": "medium",
      "validator": null
    }
  ]
}
```

**Action types:**
- `{ "type": "blur" }` — Gaussian blur over the matched region
- `{ "type": "black_box" }` — Solid black rectangle over the matched region
- `{ "type": "mask", "replacement": "[TEXT]" }` — Replace matched text in logs
- `{ "type": "drop" }` — Don't log the event at all
- `{ "type": "flag" }` — Record a flag but don't redact (for review)

**Severity levels:** `low`, `medium`, `high`, `critical`

**Validators:** `"luhn"` (Luhn checksum for credit card numbers), or `null`

---

## 9. Platform support matrix

| Feature | Windows 11 | Windows 10 | macOS | Linux (X11) | Linux (Wayland) |
|---|---|---|---|---|---|
| Screen capture | ✅ | ✅ | ✅ | ✅ | ⚠️ Partial |
| Smart change detection | ✅ | ✅ | ✅ | ✅ | ✅ |
| OCR text extraction | ✅ Native | ✅ Native | 🔄 Planned | 🔄 Planned | 🔄 Planned |
| PII redaction | ✅ | ✅ | 🔄 Planned | 🔄 Planned | 🔄 Planned |
| Window title detection | ✅ | ✅ | ✅ | ✅ | ⚠️ Limited |
| Word export | ✅ | ✅ | ✅ | ✅ | ✅ |
| All other exports | ✅ | ✅ | ✅ | ✅ | ✅ |
| Time Machine replay | ✅ | ✅ | ✅ | ✅ | ✅ |
| Chain verification | ✅ | ✅ | ✅ | ✅ | ✅ |
| Rule wizard | ✅ | ✅ | ✅ | ✅ | ✅ |

**Key notes:**
- OCR and PII redaction currently work fully on **Windows only** (uses Windows.Media.Ocr). Cross-platform OCR via the `ocrs` engine is in development.
- On macOS/Linux, the "Redact PII" toggle has no effect until cross-platform OCR ships.
- Wayland support is limited because Wayland restricts screen capture for security reasons. Use X11 or XWayland for full functionality.

---

## 10. Use cases

### 10.1 Software installation documentation

**Scenario:** You need to document how to install a new application for your IT team's knowledge base.

**Steps:**
1. Open Traceflow and enter title: "Installing Acme Pro 5.2 on Windows 11"
2. Set sensitivity to 4% (default — good for installer wizards)
3. Enable PII redaction (in case license keys or account info appear)
4. Click **Start recording**
5. Run the installer normally — Traceflow captures each wizard step automatically
6. Click **Stop recording** when done
7. Review the captured steps — edit descriptions to add context ("Click Next to accept defaults")
8. Export to Word for the knowledge base

**Result:** A polished Word document with a title page, one section per installer step, each with a screenshot and description, sensitive data automatically redacted.

### 10.2 ERP configuration procedure

**Scenario:** You're configuring SAP general ledger accounts and need to document the exact steps for compliance.

**Steps:**
1. Title: "SAP GL Account Setup — Cost Center 4200"
2. Enable the **financial** rule pack (Settings → toggle it on) to redact account numbers
3. Create a custom rule for your internal cost center format: Wizard → Custom → pattern `\bCC-\d{4}\b`
4. Start recording
5. Walk through the SAP configuration screens
6. Stop recording
7. Export to JSON for audit evidence — includes the full hash chain
8. Export to Word for the finance team

**Result:** Two outputs from one capture: a verifiable audit record (JSON) and a readable guide (Word), both with financial data redacted.

### 10.3 Quality assurance test evidence

**Scenario:** You need to prove that a software release was tested by capturing every screen during the test run.

**Steps:**
1. Title: "QA Test Run — Release 3.1.0 — Regression Suite"
2. Set FPS to 8 and sensitivity to 3% (capture everything)
3. Start recording
4. Execute the test suite — Traceflow captures every screen transition
5. Stop recording
6. Open **Time Machine** to review the exact sequence
7. Click **✓ Verify chain integrity** — the green "chain intact" message proves no steps were added or removed after the fact
8. Export to JSON as the official test evidence

**Result:** A cryptographically verifiable record of every screen seen during testing, with timestamps and event metadata.

### 10.4 Customer support procedure documentation

**Scenario:** Your support team needs standardized procedures for common customer issues.

**Steps:**
1. Title: "Resetting Customer Password — Standard Procedure"
2. Enable PII redaction with the `general_pii` and `secrets` packs
3. Add a custom rule for customer IDs: Wizard → Custom → pattern `\bCUST-\d{8}\b` → mask with `[CUSTOMER]`
4. Start recording
5. Walk through the password reset procedure in your support portal
6. Stop recording
7. Edit descriptions to add notes: "Verify customer identity before proceeding"
8. Export to HTML for the support team's intranet

**Result:** An interactive HTML guide with customer PII automatically redacted, ready to publish on the internal wiki.

### 10.5 Compliance audit preparation

**Scenario:** Your organization faces a SOX/HIPAA/ISO audit and needs to demonstrate that documented procedures were followed.

**Steps:**
1. Capture every critical procedure throughout the audit period using Traceflow
2. After each procedure, export to JSON (audit format)
3. Store the JSON exports in your audit evidence repository
4. When auditors ask for evidence:
   - Provide the JSON file
   - Open it in Traceflow and click **✓ Verify chain integrity**
   - Show the auditor the green verification message
   - Use **Time Machine** to walk through the procedure step by step

**Why it works:** The SHA-256 hash chain means the evidence is tamper-evident. If anyone modified the log — adding, removing, or changing any step — the chain would break and verification would fail. This is the same principle used by Git, blockchain, and certificate transparency logs.

### 10.6 Onboarding and training material creation

**Scenario:** You need to create training materials for new employees learning your internal tools.

**Steps:**
1. Title: "New Hire Guide — Setting Up Your Development Environment"
2. Start recording
3. Walk through the entire setup process: IDE installation, Git configuration, VPN setup, tool access requests
4. Stop recording
5. Review and edit descriptions to add helpful notes and warnings
6. Delete any steps that show irrelevant screens (e.g., waiting for downloads)
7. Export to Markdown for the team wiki
8. Export to Word for the printed onboarding packet

**Result:** Comprehensive training documentation with real screenshots, created in the time it takes to do the procedure once.

### 10.7 Incident response documentation

**Scenario:** A production incident occurs and you need to document every step of the response for the post-mortem.

**Steps:**
1. As soon as the incident begins, open Traceflow
2. Title: "Incident INC-2024-0342 — Database Failover Response"
3. Enable secrets and general_pii rule packs (incident response often touches credentials)
4. Start recording
5. Work through the incident response — every dashboard, terminal command, configuration change is captured
6. Stop recording when the incident is resolved
7. Export to JSON for the official incident record
8. Use Time Machine to prepare the timeline for the post-mortem meeting

**Result:** A timestamped, tamper-evident record of every action taken during the incident, with sensitive credentials automatically redacted.

### 10.8 Regulatory procedure validation (pharma, healthcare, finance)

**Scenario:** A pharmaceutical company needs to validate that a computer system is configured correctly per FDA 21 CFR Part 11 requirements.

**Steps:**
1. Title: "IQ/OQ Protocol — LIMS Configuration — Protocol #VAL-2024-089"
2. Enable all three rule packs (general_pii + secrets + financial)
3. Add custom rules for patient identifiers and protocol numbers specific to your organization
4. Record the entire Installation Qualification (IQ) and Operational Qualification (OQ) execution
5. Stop recording
6. Verify chain integrity — document the verification result
7. Export to JSON (machine-verifiable evidence) and Word (human-readable)
8. Attach both to the validation protocol documentation

**Why it matters for regulated industries:**
- **Tamper evidence:** The hash chain provides cryptographic proof that the record hasn't been modified — a key requirement for audit trails.
- **Automated redaction:** Patient data (PHI) and financial data (PII) are automatically removed, reducing the risk of accidental disclosure.
- **Offline operation:** No cloud dependency means the tool can be used in validated environments without network connectivity — critical for air-gapped systems.
- **Reproducible evidence:** The JSON export contains everything needed to independently verify the record: events, hashes, and chain links.

### 10.9 Multi-language documentation

**Scenario:** Your company operates in multiple countries and needs documentation in English and Spanish.

**Steps:**
1. Capture the workflow once in Traceflow
2. Export to Markdown
3. Edit the step descriptions in English, export to Word
4. Edit the same step descriptions in Spanish, export again
5. Both documents share identical screenshots — only the text differs

**Future enhancement:** Automatic translation of step descriptions is planned for a future release.

### 10.10 Process mining and improvement

**Scenario:** You want to understand how different team members perform the same procedure and identify variations.

**Steps:**
1. Have three team members each capture the same procedure using Traceflow
2. Export all three as JSON
3. Compare the step counts, timing, and window sequences
4. Identify where procedures diverge — one person takes 12 steps, another takes 8
5. Standardize on the most efficient workflow
6. Document the standard procedure using Traceflow's Word export

**Future enhancement:** Automated workflow comparison and process graph visualization are planned for Sprint 3+.

---

## 11. Troubleshooting

### The app starts but the window is blank

Open DevTools (Ctrl+Shift+I), check the Console for CSP errors. Verify `tauri.conf.json` has `asset:` in the `security.csp.img-src` directive.

### Capture starts but no steps appear

- Check that you're switching between applications or performing visible actions. Traceflow only captures when the screen changes meaningfully.
- Lower the sensitivity threshold in settings (try 2-3%).
- Check the terminal/PowerShell window for backend errors.

### Steps are captured but screenshots are black

This can happen when:
- The application uses hardware-accelerated rendering (some browsers in fullscreen mode)
- A game or video player is running in exclusive fullscreen
- Remote Desktop is active with certain display drivers

**Fix:** Try switching the application to windowed mode, or disable hardware acceleration in the application's settings.

### PII redaction is enabled but nothing is being redacted

- On **macOS/Linux:** PII redaction requires OCR, which currently only works on Windows. The toggle has no effect on other platforms.
- On **Windows:** Verify that rule packs are enabled in Settings. Check that at least one pack (e.g., `general_pii`) is toggled on.
- Verify the data you expect to be redacted matches a pattern in the active rules. Use the live regex tester to confirm.

### Export produces an empty document

Verify the session has steps: check the step count in the status bar. If it shows "0 steps," the capture didn't detect any changes — see "Capture starts but no steps appear" above.

### Chain verification fails

This means the event log was modified after capture. Possible causes:
- Manual editing of `events.ndjson` (don't do this)
- Disk corruption
- A bug in a third-party tool that modified the file

**Action:** The session's integrity can no longer be guaranteed. Re-capture if possible. Report to your compliance team if this was an audit-critical session.

### The wizard shows no preset cards

The Settings panel failed to load data from the backend. Check the DevTools Console (Ctrl+Shift+I) for red error messages. The most common cause is an IPC command name mismatch — verify all commands are registered in `lib.rs`.

### Stop button doesn't respond

If clicking Stop does nothing and the Console shows "handleStop called" but nothing after, the backend's stop command is hanging. This was fixed in version 0.2.0+. Ensure you're running the latest version.

### The app uses too much CPU

- Lower the FPS setting from 8 or 4 to 2.
- Increase the sensitivity threshold so fewer frames trigger captures.
- The adaptive throttling system automatically reduces the poll rate when CPU usage is high, but lowering the base FPS helps more.

### Session folder is very large

Each captured step stores a full-resolution PNG (~1–5 MB depending on screen resolution). A 50-step session at 4K resolution may use 50–250 MB.

**To reduce size:**
- Delete unnecessary steps before exporting
- Disable forensic mode (`keep_all_frames = false` in config) — this prevents saving sub-threshold frames
- Capture at 1080p instead of 4K if ultra-high resolution isn't needed

---

## 12. Glossary

| Term | Definition |
|---|---|
| **Capture session** | One recording run from Start to Stop, producing an event log and captured frames |
| **Chain** | The SHA-256 linked list of event records; each record references the hash of the previous |
| **Content-addressed** | Frame files named by their SHA-256 hash; identical screenshots produce identical files |
| **Event log** | The append-only NDJSON file (`events.ndjson`) that records everything that happens during a session |
| **Frame** | A captured screenshot, stored as a PNG in the `frames/` directory |
| **Hash chain** | See "Chain" — the tamper-detection mechanism linking all events |
| **NDJSON** | Newline-delimited JSON — one JSON object per line, easy to stream and process |
| **OCR** | Optical Character Recognition — extracting text from screenshot images |
| **PII** | Personally Identifiable Information — data that can identify an individual (emails, phone numbers, IDs) |
| **Projection** | Building a derived view (like a step list or document) from the event log |
| **Promotion** | The moment a candidate frame passes the stability check and becomes a captured step |
| **Rule pack** | A JSON file containing patterns (regex) and actions (blur, mask, etc.) for PII detection |
| **Sensitivity** | The change threshold — how much the screen must differ to trigger a capture |
| **Step** | One promoted frame in a session — a meaningful screen state that was captured |
| **Tamper-evident** | A property of the hash chain: any modification to the log is detectable |
| **Time Machine** | The replay viewer that lets you scrub through a session's timeline |
| **Wizard** | The preset-based rule creation flow — pick a common pattern, customize it, save |

---

## Appendix A: Keyboard shortcuts

| Shortcut | Action |
|---|---|
| Ctrl+Shift+I | Open DevTools (for debugging) |

*Additional keyboard shortcuts are planned for a future release.*

## Appendix B: File format reference

### events.ndjson

Each line is a JSON object:

```json
{
  "seq": 0,
  "at": "2026-06-10T10:30:00Z",
  "session": "a1b2c3d4-...",
  "prev": "0000000000000000000000000000000000000000000000000000000000000000",
  "hash": "abc123...",
  "body": {
    "kind": "session_start",
    "title": "My Procedure",
    "capture_settings": { ... },
    "app_version": "0.2.0",
    "host_os": "windows"
  }
}
```

### Frame files

- Location: `sessions/<uuid>/frames/<sha256>.png`
- Format: PNG, full screen resolution
- Naming: SHA-256 hash of the file's bytes
- Content: Post-redaction (if PII redaction was enabled)

---

*End of user manual. For developer documentation, see `README.md`, `ENGINEERING_STANDARDS.md`, and `CLAUDE.md`.*
