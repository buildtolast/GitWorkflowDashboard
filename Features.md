# Features

## Native Desktop UI

- Built with [Slint](https://slint.dev) — declarative Rust-native UI framework
- 15 `.slint` component files with full theming support
- Tokio async backend on a separate thread, connected via mpsc command channel
- Broadcast channel pushes poller state updates to UI thread via `invoke_from_event_loop`
- Web UI code preserved behind `web` Cargo feature for backward compatibility

## Dashboard Tabs

### Active Runs
- Live view of all in-progress and queued workflow runs across repos
- Expandable run cards with job and step details
- Cancel and re-run buttons on each card
- "View Logs" opens the collapsible log viewer panel

### Workflows
- Searchable table of all workflow definitions across monitored repos
- Shows workflow name, file path, repo, and state

### Actions
- Searchable card grid of all dispatchable workflows
- One-click **Quick Run** button triggers on default branch
- **Configure** opens dispatch dialog with branch selector and typed input forms

### History
- Searchable completed runs with conclusion status (success, failure, cancelled)
- Time range filters: All, 1h, 6h, 12h, 3d, 7d, custom date picker
- Expandable run detail inline with full job log support
- Re-run button to re-trigger any historical workflow

### What's Happening
- Activity overview with aggregate stats (total, success, failure, cancelled, in-progress, queued)
- Per-repo breakdown with success rate bars
- Per-workflow breakdown

### Analytics
- **Actor Leaderboard** — Top contributors ranked by run count with inline bars
- **Success Rates** — Per-repo success rate with color-coded progress bars
- **Duration Analysis** — Average/min/max build times per workflow
- Time range filters: 1h, 6h, 12h, 3d, 7d

## Job Log Viewer
- Collapsible bottom panel for job logs, hidden by default
- Works from both Active Runs and History tabs
- ANSI escape code stripping for clean display
- Step-level log grouping via `##[group]`/`##[endgroup]` markers
- 2MB log truncation with indicator
- Handles expired logs gracefully

## Workflow Dispatch
- Trigger `workflow_dispatch` workflows from the dashboard
- Branch selector via ComboBox (fetches branches from GitHub API)
- Auto-discovers workflow inputs by parsing YAML via GitHub Contents API
- Typed input forms: string, boolean (checkbox), choice (dropdown), number
- Input validation with descriptions, required flags, and default values

## Multi-Repo Management
- Monitor workflows across multiple repositories simultaneously
- Search GitHub repos by name via GitHub Search API
- Bulk-select and add multiple repos from search results
- Add/remove repos at runtime
- Sidebar with repo list and click-to-select filtering
- Persistent repo list saved to disk (JSON)
- Maximum monitored repos cap (configurable, default 50)

## Real-Time Updates
- Broadcast channel for instant state changes from poller to UI
- Background poller with configurable interval (10-300 seconds)
- Smart polling: ETag conditional requests (304s save rate limit)
- Auto-skip polls when rate limit is critically low (< 10 remaining)
- Manual refresh button in header

## Themes
- 5 built-in themes: Terminal Dark (default), Light, Monokai, Solarized Dark, Midnight Blue
- Theme selector dropdown in the header (ComboBox)
- Persists across sessions via file (`~/.gwd/theme`)
- All colors defined via `ThemeColors` struct applied through Slint global

## Toast Notifications
- Success, error, and info toast messages
- Positioned at top-right with dismiss buttons
- Color-coded left accent stripe per toast type
- Max 5 toasts displayed simultaneously

## Deployment
- **Single binary** — Rust with embedded Slint UI
- **Docker** — Multi-stage build (Rust 1.93), primarily for CI
- **macOS DMG** — One-command DMG build with `.app` bundle
  - Universal binary support (Intel + Apple Silicon)

## Observability
- Rate limit badge with live remaining/total count and color coding
- Connection status indicator (connected/disconnected)
- Last poll timestamp display
- Structured logging via `tracing` with optional JSON output (`LOG_FORMAT=json`)

## Technical
- **UI:** Slint 1.9+ declarative markup with Rust bindings
- **Backend:** Rust (tokio async runtime, reqwest with rustls)
- **Architecture:** UI thread (Slint) ↔ mpsc channel ↔ tokio thread (poller + command handler)
- **State:** `Arc<RwLock<T>>` for shared dashboard state and repo list
- **Broadcast:** `tokio::sync::broadcast` channel for poller→UI updates
- **Tests:** 160 tests (60 lib + 100 integration)
- **Feature gating:** actix-web deps behind `web` Cargo feature
