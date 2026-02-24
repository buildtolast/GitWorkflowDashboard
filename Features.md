# Features

## Dashboard Tabs

### Active Runs
- Live view of all in-progress and queued workflow runs across repos
- Expandable run cards with job and step details
- Real-time duration counters for running jobs
- Cancel and re-run buttons on each card
- "View Logs" opens the split-panel log viewer at the bottom of the dashboard

### Workflows
- Searchable table of all workflow definitions across monitored repos
- Shows workflow name, file path, repo, and state
- Collapsible groups by repository with expandable/collapsible parent headers
- Alternating group colors (cyan odd, purple even) for visual distinction
- Search auto-expands matching groups, all groups default collapsed
- Single Expand All / Collapse All toggle button

### Actions
- Searchable card grid of all dispatchable workflows
- One-click **Run** button triggers on default branch
- **Configure & Run** opens a modal with branch selector and typed input forms

### History
- Searchable completed runs with conclusion status (success, failure, cancelled)
- Time range filters: 1h, 6h, 12h, 3d, 7d, custom date picker
- "Details" button expands run detail inline with full job log support
- Re-run button to re-trigger any historical workflow

### What's Happening
- Activity overview with aggregate stats
- Per-repo and per-workflow breakdown
- Run timeline visualization

### Analytics
- **Success Rate** — Overall percentage hero display, per-repo and per-workflow bar charts sorted by rate
- **Duration Analysis** — Average/median build times, trend detection (getting faster/slower), per-workflow averages, slowest runs table
- **Trigger Analysis** — Breakdown by event type (push, PR, dispatch, schedule) with run counts and success rates
- **Actor Leaderboard** — Top contributors ranked by run count with inline success rate bars and failure counts
- **Branch Activity** — Most active branches with stacked success/failure/cancelled bar charts
- **Failure Analysis** — Most failing workflows, failure streak detection (active streaks highlighted), recent failures table
- Time range filters: 1h, 6h, 12h, 3d, 7d (default), custom date picker
- All metrics computed client-side from existing polling data (no additional API calls)

## Job Log Viewer
- View GitHub Actions job logs in a dedicated split-panel viewer at the bottom of the dashboard
- Panel is hidden by default, appears only when "View Logs" is clicked on an expanded job
- Works from both Active Runs and History tabs
- ANSI escape code stripping for clean display
- Step-level log grouping via `##[group]`/`##[endgroup]` markers
- Timestamp and directive prefix stripping (`##[debug]`, `##[warning]`, `##[error]`, `##[command]`)
- 2MB log truncation with indicator to prevent memory issues
- Handles expired logs (older than 90 days) gracefully

## Workflow Dispatch
- Trigger `workflow_dispatch` workflows from the dashboard
- Searchable branch selector with smart sorting (main/master/develop first)
- Auto-discovers workflow inputs by parsing YAML via GitHub Contents API
- Typed input forms: string, boolean (checkbox), choice (dropdown), number, environment
- Input validation with descriptions, required flags, and default values

## Multi-Repo Management
- Monitor workflows across multiple repositories simultaneously
- Search GitHub repos by name, org, or topic via GitHub Search API
- Bulk-select and add multiple repos from search results
- Add/remove repos at runtime without restarting the server
- Sidebar with repo list and click-to-select filtering
- GitHub link button on each sidebar repo (opens in new tab)
- Copy URL button on each sidebar repo (copies HTTPS URL to clipboard)
- Auto-fit sidebar width to longest repository name (clamped 200–480px)
- Hover tooltip for truncated repo names
- "Loading repositories..." spinner while waiting for first poll
- Maximum monitored repos cap (configurable, default 50)
- Persistent repo list saved to `~/.gwd/repos.json` with toast confirmation on add/remove

## Real-Time Updates
- WebSocket broadcast channel for instant state changes
- Auto-reconnect with exponential backoff on disconnect
- GitHub webhook receiver (HMAC-SHA256 verified) for instant updates
- Background poller with configurable interval (10-300 seconds)
- Smart polling: ETag conditional requests (304s don't count against rate limit)
- Auto-skip polls when rate limit is critically low (< 10 remaining)
- Manual refresh button in header or via `POST /api/refresh`

## Themes
- 5 built-in themes: Terminal Dark (default), Light, Monokai, Solarized Dark, Midnight Blue
- Theme selector dropdown in the header
- Persists across sessions via localStorage
- Auto-detects OS light/dark preference for first-time visitors
- Listens for OS theme changes in real-time
- All colors use CSS custom properties for full theme coverage

## Security
- Bearer token authentication for all `/api/*` endpoints (`--dashboard-token`)
- CSRF protection via `X-Requested-With` header validation
- Rate limiting (~60 requests/min per IP via `actix-governor`)
- CORS lockdown: explicit origin allowlist (`--cors-origins`)
- Security headers: CSP, X-Frame-Options DENY, X-Content-Type-Options nosniff, Referrer-Policy
- XSS prevention: `escJs()` for JS contexts, `safeOpenUrl()` for URL validation
- Error message sanitization: internal details never leaked to HTTP clients
- Sensitive field redaction in debug output
- WebSocket authentication via query param token
- WebSocket connection limit (max 50 concurrent)
- ETag cache size cap (max 1000 entries with 10% eviction)
- Frontend auth dialog with sessionStorage token management

## Startup
- Full-screen initialisation overlay with animated hero SVG dashboard illustration
- Progress bar with 3 status steps: Connect → Repos → Ready
- Responsive sizing via CSS `clamp()` to fit all screen resolutions
- Rotating status messages with elapsed timer during long waits
- "Take me to Dashboard" skip button appears after 10 seconds
- 3-second fallback dismisses overlay when no repos are configured
- Overlay waits for sidebar to be visually populated before dismissing

## Deployment
- **Single binary** — Rust backend with embedded HTML frontend via `include_str!`
- **Docker** — Multi-stage build, single binary runtime image
- **macOS DMG** — One-command DMG build with `.app` bundle
  - Universal binary support (Intel + Apple Silicon)
  - Install/Uninstall scripts included
  - Token prompt on first launch with config persistence
  - Auto-opens browser to dashboard
- **run.sh** — Convenience script for source, Docker, DMG, build, clean, and stop

## Observability
- Rate limit badge with live remaining/total count and color coding
- WebSocket connection indicator (connected/connecting/disconnected)
- Last poll timestamp display
- Structured logging via `tracing` with optional JSON output (`LOG_FORMAT=json`)
- Graceful shutdown via CancellationToken on SIGINT

## Technical
- **Backend:** Rust (actix-web 4, reqwest with rustls, tokio async runtime)
- **Frontend:** Single HTML file (~5600 lines) with embedded CSS + vanilla JS
- **State:** `Arc<RwLock<T>>` for shared dashboard state and repo list
- **Broadcast:** `tokio::sync::broadcast` channel for WebSocket fan-out
- **Tests:** 160 tests (60 lib + 100 integration)
- **No external frontend dependencies** — no Node.js, npm, or build toolchain
