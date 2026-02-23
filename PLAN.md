# Plan: Slint Native UI Migration

**Status: COMPLETED (2026-02-23)**

Migrated the entire UI from embedded HTML/CSS/JS (actix-web) to a native Slint Rust desktop application. All existing features preserved.

## What was implemented
- 15 Slint component files: theme, header, sidebar, tab_bar, active_runs, workflows, actions_tab, history, whats_happening, analytics, dispatch_dialog, log_viewer, toast, branches (stub), widgets
- 4 Rust UI modules: backend.rs (tokio bridge), actions.rs (command handlers), bridge.rs (type conversions), theme.rs (5 palettes + persistence)
- Rewrote main.rs as Slint entry point with callback wiring and mpsc command channel
- Feature-gated all actix-web code behind `web` Cargo feature (error.rs, github/webhook, integration tests)
- Thread safety: ModelRc (Rc-based) created on UI thread only, plain data passed across thread boundary
- Updated Dockerfile: Rust 1.93, new module stubs, build.rs touch for Slint recompilation
- 160 tests pass (60 lib + 100 integration)

## Architecture
```
Slint UI (main thread) ↔ mpsc::UnboundedChannel<UiCommand> ↔ tokio runtime (background thread)
                                                                    ↕
                                                         broadcast::channel<WsMessage> (poller → UI)
```

## Key patterns
- `slint::include_modules!()` in lib.rs (not main.rs) so all modules access generated types
- `invoke_from_event_loop` for thread-safe UI updates from async code
- `Weak<MainWindow>` passed to background thread (Clone + Send safe)
- All ModelRc construction inside `invoke_from_event_loop` closures

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
