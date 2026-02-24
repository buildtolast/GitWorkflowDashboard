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
