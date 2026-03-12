# Plan: v2.9 Frontend Modularization

**Status: COMPLETED (2026-02-26)**

Split the monolithic single-file frontend (`src/frontend/index.html`, 9534 lines) into 23 modular source files assembled by `build.rs` at compile time. Zero runtime change — the server still serves one embedded HTML string.

## What was implemented

### Build System (`build.rs`)
- Extended `build.rs` to concatenate shell fragments + CSS + JS files into `$OUT_DIR/index.html`
- Assembly order: `shell_head.html` → 12 CSS files → `shell_mid.html` → 8 JS files → `shell_foot.html`
- Added `rerun-if-changed` watchers for all 23 source files
- `mod.rs` updated to `include_str!(concat!(env!("OUT_DIR"), "/index.html"))`

### Shell Fragments
- `shell_head.html` (10 lines) — DOCTYPE, `<head>`, meta, title, `<style>` opener
- `shell_mid.html` (549 lines) — `</style></head><body>` + all HTML body markup + `<script>` opener
- `shell_foot.html` (3 lines) — `</script></body></html>`

### CSS Files (`src/frontend/styles/`)
| File | Lines | Content |
|------|-------|---------|
| `01-variables.css` | 364 | CSS vars, 5 themes, reset, scrollbar |
| `02-layout.css` | 188 | Header, sidebar+main body layout |
| `03-sidebar.css` | 447 | Sidebar, build info, add repo, resize |
| `04-main-content.css` | 275 | Tab bar, tab content, log viewer |
| `05-active-runs.css` | 496 | Run cards, badges, expanded, step logs |
| `06-workflows.css` | 189 | Workflow tab + checkboxes |
| `07-pipeline.css` | 366 | Custom tabs, stages, variables sidebar |
| `08-tabs-misc.css` | 549 | Actions, History, What's Happening |
| `09-analytics.css` | 227 | Analytics tab |
| `10-components.css` | 758 | Modal, toasts, splash, scanline |
| `11-accessibility.css` | 116 | Focus-visible, log modal |
| `12-responsive.css` | 56 | Media queries |

### JS Files (`src/frontend/js/`)
| File | Lines | Content |
|------|-------|---------|
| `01-state.js` | 65 | State object + constants |
| `02-utils.js` | 632 | Utilities, Slack helpers, inputs cache |
| `03-websocket.js` | 336 | WebSocket connect, reconnect, handlers |
| `04-render.js` | 938 | Render functions (repos, runs, workflows) |
| `05-analytics.js` | 372 | Analytics rendering |
| `06-app.js` | 2450 | App object (all actions, pipeline, Slack) |
| `07-ui.js` | 64 | Sidebar resize, modal close, timers |
| `08-init.js` | 84 | Initialization IIFE |

### Verification
- `cargo build` + `cargo test` — 273 tests pass
- Byte-for-byte HTML equivalence verified via `diff` against original
- Old `src/frontend/index.html` (9534 lines) removed via `git rm`

## Commit
- `ae926da` — Modularize frontend: split monolithic HTML into 23 source files

---

# Plan: v2.8 Feature Port & Pipeline Orchestration

**Status: COMPLETED (2026-02-25)**

Selectively ported high-value features from `feature/added-functionality` branch (6 commits) while preserving all existing main-branch capabilities (themes, auth, analytics, splash, live log tailing).

## What was ported

### Backend (Rust)
- **Slack webhook relay** — `POST /api/integrations/slack/message` with HTTPS-only `hooks.slack.com` validation (`src/handlers/integrations.rs`)
- **Enhanced branch listing** — `list_branches()` with deduplication and default branch fallback (`src/github/client.rs`)
- **Plain-text job logs** — `GET /api/runs/{o}/{r}/jobs/{job_id}/logs/plain` with 250K char truncation, simpler variant for orchestration UI (`src/handlers/runs.rs`, `src/github/client.rs`)

### Frontend (index.html — +1628 lines)
- **Step-level inline log viewer** — per-step "View Logs" buttons with timestamp-based extraction (`extractStepLogFromJobLogs()`), expandable in-place panels, full-screen modal
- **Repo persistence in localStorage** — repos survive page reloads via `gwd_saved_repos` key with save/load/infer helpers
- **Branch caching** — `getDefaultBranchForRepo()` reduces API calls by caching default branch per repo (`State.branchCache`)
- **Focus-visible accessibility** — 2px cyan outlines on interactive elements for keyboard navigation
- **Sticky workflow controls** — workflow toolbar stays pinned on scroll; tab panels fade in smoothly
- **Custom Workflow Tabs** — full pipeline orchestration system:
  - Select workflows via checkboxes, create named tabs
  - Drag-to-reorder tabs, assign workflows to numbered stages
  - Variables sidebar for per-pipeline branch and input config
  - Sequential plan execution (parallel within stage)
  - Inter-stage gates: manual approval and Slack notifications
  - Export/import tab configurations as JSON
  - LocalStorage persistence (`gwd_custom_workflow_tabs` key)
- **Slack Template Library** — full Slack notification system:
  - Create, edit, delete reusable message templates with `{{variable}}` substitution
  - Multi-URL webhook management persisted to localStorage
  - Template-based before/after stage notifications via dropdown selectors
  - Boundary stages (before first, after last) support
  - Pipeline execution sends templated Slack messages at configured points
- **Workflow Input Parameters** — auto-detected from `workflow_dispatch`:
  - `ensureWorkflowInputsCached()` fetches input definitions from API
  - Typed input rendering: string, boolean (checkbox), choice (dropdown), environment
  - Input descriptions, required markers, default values in variables sidebar
  - Per-workflow input overrides saved to tab config

## Commits
1. `ff1e772` — Slack webhook relay + enhanced `list_branches()`
2. `307bdcd` — Plain-text job logs endpoint
3. `c6cde8c` — Frontend: 6 features (step logs, repo persistence, branch cache, a11y, sticky controls, custom workflow tabs)
4. `d6c48f5` — PLAN.md update
5. `7f4ff01` — PLAN.md v2.8 backend + frontend sections
6. `baf83c0` — Full Slack template system + workflow input parameters

---

# Plan: v2.7 Architecture Improvements

**Status: COMPLETED (2026-02-24)**

Exponential backoff, SQLite persistence, and server-side analytics — based on external architectural analysis.

## What was implemented
- **Exponential backoff with jitter**: Adaptive polling interval based on rate limit status. Three tiers: normal (>100 remaining), graduated backoff (10-100, formula: `base × 2^n + jitter`), critical skip (<10). Auto-recovery when rate limit replenishes. 5-minute max cap.
- **SQLite persistence layer**: Replaced `~/.gwd/repos.json` with `~/.gwd/dashboard.db` (rusqlite bundled). WAL mode, auto-migration from JSON, full workflow run storage with upsert on every poll cycle. Repo CRUD via database transactions.
- **Server-side analytics API**: `GET /api/analytics/summary?range=7d` endpoint. Pre-computed via SQL: success rates, duration analysis, trigger breakdown, actor leaderboard, failure analysis, branch activity. 60s response cache. Supports 1h/6h/12h/1d/3d/7d/30d ranges.
- 273 tests pass (62 lib + 106 binary + 105 integration)

## What was NOT implemented (and why)
- GraphQL migration: massive refactoring, ETag caching already minimizes API calls
- Octocrab library: reqwest works well, custom client is well-tested
- Actix Actor model for WS: over-engineering for current scale
- Cargo workspace: single crate is fine for project size
- Virtualized log rendering: 2MB truncation handles this
- Rust WASM frontend: already migrated to Slint native

---

# Plan: v2.6 Web UI Enhancements

**Status: COMPLETED (2026-02-23)**

Collapsible workflow groups, startup splash screen, sidebar enhancements, and repo persistence messaging.

## What was implemented
- Collapsible workflow groups by repo with cyan/purple alternating colors, search auto-expand, single Expand/Collapse All toggle
- Startup splash screen with animated hero SVG, progress steps (Connect → Repos → Ready), responsive `clamp()` sizing
- Splash screen UX: rotating status messages, elapsed timer, "Take me to Dashboard" skip button, 3s fallback for no-repos
- Overlay waits for sidebar to render (double `requestAnimationFrame`) before dismissing
- Sidebar: GitHub link + copy URL buttons, auto-fit width (200–480px), hover tooltips, "Loading repositories..." spinner
- Repo persistence messaging: toast messages show `~/.gwd/repos.json` path on add/remove

---

# Plan: Multi-Theme System

**Status: COMPLETED (2026-02-20)**

5 themes (Terminal Dark, Light, Monokai, Solarized Dark, Midnight Blue) with auto OS preference detection, localStorage persistence, and no-transition flash prevention.

## What was implemented
- Converted ~47 hardcoded RGBA values to ~28 `--tint-*` CSS custom properties
- 4 theme override blocks (`[data-theme="light|monokai|solarized|midnight"]`), ~60 vars each
- Theme selector `<select>` dropdown in header-right
- `App.setTheme(themeId)` with `localStorage` persistence (`gwd_theme` key)
- OS preference auto-detection via `matchMedia('prefers-color-scheme')` for first-time visitors
- `html.no-transition` class prevents CSS transition flash on page load
- Light theme SVG dropdown arrow override for visibility on light backgrounds
- `card-pulse` keyframe left hardcoded (CSS keyframes have limited variable support cross-browser)

---

# Plan: Inline Workflow Job Logs

**Status: COMPLETED (2026-02-20)**

View GitHub Actions job logs directly in the dashboard with ANSI stripping, step-level parsing, and client-side caching.

## What was implemented
- Backend: `LogsExpired` error variant, `JobLogStep`/`JobLogResponse` models, `get_job_log()` client method with ANSI stripping + step parsing, handler + route
- Frontend: `State.jobLogs` cache, CSS styles, "View Logs" button on expanded jobs, `fetchJobLog()` method
- History tab: "Details" button for inline run expansion with log support
- 120 tests pass (36 lib + 47 binary + 37 integration)

---

# Plan: Branches Feature

**Status: DISABLED (2026-02-20)**

Branches tab and all branch API routes temporarily disabled. Code preserved with `#[allow(dead_code)]` for future re-enablement.

## What was disabled
- Frontend: Branches tab button (HTML-commented out)
- Backend: `/api/branches/stale`, `/api/branches/{o}/{r}`, `DELETE /api/branches/{o}/{r}/{branch}` routes (commented out in main.rs)
- All branch-related code kept intact with `#[allow(dead_code)]` annotations

## To re-enable
1. Uncomment tab button in `src/frontend/index.html` (search "Branches tab disabled")
2. Uncomment routes in `src/main.rs` (search "will revisit later")
3. Remove `#[allow(dead_code)]` from: `handlers/mod.rs`, `config.rs`, `github/client.rs`, `handlers/workflows.rs`, `models.rs`
