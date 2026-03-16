# Release Notes

## v3.0 (2026-03-16)

### Cmd+K Command Bar
- Natural language workflow dispatch via `Cmd+K` / `Ctrl+K` floating command bar
- Fuzzy search across all dispatchable workflows
- Keyboard navigation (arrow keys, Enter, Escape)
- Event-delegated click handling to fix blur-race on result selection

### 5-Minute Video Presentation
- AI-narrated intro video (`slides/GitWorkflowDashboard_Intro.mp4`) with ElevenLabs TTS
- 10 HTML presentation slides + 7 live demo screenshots
- Whisper-based audio/slide sync with 1-second early slide transitions
- Presentation scripts (`presentation-script.txt`, `presentation-script-5min.txt`)
- `audio-video.md` — reusable checklist for producing future presentation videos

### Technical
- Frontend source files: 23 → 25 (added `09-commandbar.js`, `13-commandbar.css`)
- 273 tests pass (62 lib + 106 binary + 105 integration)

---

## v2.9 (2026-02-26)

### Frontend Modularization
- Split monolithic single-file frontend (9534 lines) into 23 modular source files
- `build.rs` assembles shell fragments + 12 CSS files + 8 JS files into `$OUT_DIR/index.html` at compile time
- Zero runtime change — server still serves one embedded HTML string via `include_str!`
- Each CSS/JS file maps to a logical concern (variables, layout, sidebar, websocket, render, app, etc.)
- `rerun-if-changed` watchers on all source files for incremental rebuilds
- Old `index.html` removed; new files are individually editable without exhausting AI context windows

### Technical
- 273 tests pass (62 lib + 106 binary + 105 integration)
- Byte-for-byte HTML output equivalence verified against original

---

## v2.8 (2026-02-25)

### Pipeline Orchestration (Custom Workflow Tabs)
- Select workflows via checkboxes, create named tabs with drag-to-reorder
- Assign workflows to numbered stages for sequential execution (parallel within stage)
- Variables sidebar for per-pipeline branch selection and input overrides
- Inter-stage gates: manual approval prompts and Slack notifications
- Export/import tab configurations as JSON
- LocalStorage persistence (`gwd_custom_workflow_tabs` key)

### Slack Template Library
- Create, edit, delete reusable message templates with `{{variable}}` substitution
- Multi-URL webhook management persisted to localStorage
- Template-based before/after stage notifications via dropdown selectors
- Boundary stages (before first, after last) support
- Pipeline execution sends templated Slack messages at configured points

### Workflow Input Parameters
- Auto-detects `workflow_dispatch` inputs from workflow YAML via API
- Typed input rendering in variables sidebar: string, boolean (checkbox), choice (dropdown), environment
- Input descriptions, required markers, and default values displayed
- Per-workflow input overrides saved to tab configuration

### Step-Level Inline Log Viewer
- Per-step "View Logs" buttons with timestamp-based extraction (`extractStepLogFromJobLogs()`)
- Expandable in-place log panels with full-screen modal option
- Plain-text job logs endpoint (`GET /api/runs/{o}/{r}/jobs/{job_id}/logs/plain`, 250K char truncation)

### Backend Additions
- Slack webhook relay — `POST /api/integrations/slack/message` with HTTPS-only `hooks.slack.com` validation
- Enhanced `list_branches()` with deduplication and default branch fallback

### UX Improvements
- Repo persistence in localStorage (repos survive page reloads via `gwd_saved_repos` key)
- Branch caching (`getDefaultBranchForRepo()`) reduces API calls
- Focus-visible accessibility: 2px cyan outlines on interactive elements for keyboard navigation
- Sticky workflow controls bar (stays pinned on scroll)
- Tab panel fade-in transitions

### Technical
- Ported from `feature/added-functionality` branch (6 commits)
- 273 tests pass (62 lib + 106 binary + 105 integration)

---

## v2.7 (2026-02-24)

### Exponential Backoff with Jitter
- Adaptive polling interval based on GitHub rate limit status
- Three-tier rate limit response: normal (>100), backoff (10-100), critical skip (<10)
- Formula: `base × 2^n + random_jitter` with 5-minute cap
- Automatic recovery when rate limit replenishes
- Jitter prevents thundering herd on multi-instance deployments

### SQLite Persistence Layer
- Replaced flat JSON (`repos.json`) with SQLite database (`~/.gwd/dashboard.db`)
- WAL mode enabled for concurrent reads during polling
- Auto-migrates existing `repos.json` on first run (renames to `.bak`)
- Stores all workflow runs with full metadata (actor, branch, duration, etc.)
- Run upsert on every poll cycle — survives server restarts
- Repo add/remove persisted via database transactions

### Server-Side Analytics API
- New endpoint: `GET /api/analytics/summary?range=7d`
- Pre-computed analytics via SQL queries against SQLite
- Supports time ranges: 1h, 6h, 12h, 1d, 3d, 7d, 30d
- Returns: success rates, duration analysis, trigger breakdown, actor leaderboard, failure analysis, branch activity
- 60-second response cache to reduce computation overhead
- Frontend can now consume pre-computed data instead of computing client-side

### Technical
- Added `rusqlite` (bundled SQLite) and `rand` dependencies
- 273 tests pass (62 lib + 106 binary + 105 integration)
- New test coverage: 14 database tests, 8 backoff tests, 5 analytics handler tests

---

## v2.6 (2026-02-23)

### Collapsible Workflow Groups
- Workflow table rows grouped by repository with expandable/collapsible parent headers
- Alternating group colors (cyan odd, purple even) for visual distinction
- Search auto-expands matching groups, all groups default collapsed on tab open
- Single Expand All / Collapse All toggle button

### Startup Splash Screen
- Full-screen initialisation overlay with animated hero SVG dashboard illustration
- Progress bar with 3 status steps: Connect → Repos → Ready
- Responsive sizing via CSS `clamp()` to fit all screen resolutions
- Rotating status messages with elapsed timer during long waits
- "Take me to Dashboard" skip button appears after 10 seconds
- 3-second fallback timer dismisses overlay when no repos are configured
- Overlay waits for sidebar to be visually populated before dismissing

### Sidebar Enhancements
- GitHub link button on each repo (opens in new tab)
- Copy URL button on each repo (copies `https://github.com/{repo}` to clipboard)
- Auto-fit sidebar width to longest repository name (clamped 200–480px)
- Hover tooltip for truncated repo names
- "Loading repositories..." spinner while waiting for first poll

### Repo Persistence Messaging
- Toast messages show save path (`~/.gwd/repos.json`) on add/remove
- Init overlay references local persistence path

---

## v2.5 (2026-02-21)

### Analytics Tab
- New Analytics tab with 6 diagnostic panels computed client-side from existing data
- **Success Rate**: overall hero percentage, per-repo and per-workflow bar charts
- **Duration Analysis**: average/median times, trend detection, per-workflow averages, slowest runs
- **Trigger Analysis**: breakdown by event type with success rates
- **Actor Leaderboard**: top contributors ranked by run count with inline success rate bars
- **Branch Activity**: most active branches with stacked success/failure bars
- **Failure Analysis**: most failing workflows, failure streak detection, recent failures list
- Time range filters (1h, 6h, 12h, 3d, 7d default, custom)

### UX Improvements
- Auto-refresh after workflow dispatch and re-run (switches to Active Runs tab)
- Expanded history rows highlighted with cyan border and tinted background
- Increased font sizes across all themes for better readability
- Log viewer: spinner with auto-retry every 5s for in-progress jobs
- Log viewer: resizable panel (drag top border), collapsible (click header)
- Sidebar: search/manual section moved to top, collapsible
- `run.sh --build` now builds release binary + Docker (was DMG)
- `run.sh --restart` command for quick rebuild + restart

### Bug Fixes
- **BUG-006**: In-progress job logs now show "waiting" spinner instead of misleading "Logs have expired"
- Added `LogsNotAvailable` error variant distinguishing 404 (not ready) from 410 GONE (expired)

---

## v2.4 (2026-02-20)

### Split Panel Log Viewer
- Logs now display in a dedicated collapsible bottom panel instead of inline within run cards
- Panel is hidden by default, appears only when "View Logs" is clicked on an expanded job
- Panel header shows job name with a close (✕) button
- Works from both Active Runs and History tabs (global panel below tab content)
- Clicking "View Logs" on a different job instantly switches the panel content
- "Viewing Logs" button state indicates which job is currently displayed
- Panel scrolls independently and caps at 45% viewport height

### Bug Fixes
- **BUG-004:** Fixed `get_run_detail()` error response leaking internal GitHub API URLs to clients
- **BUG-005:** Fixed `get_job_log()` returning misleading "Repository not found" for job log 404s — now returns "Logs have expired"

---

## v2.3 (2026-02-20)

### Multi-Theme System
- 5 themes: Terminal Dark (default), Light, Monokai, Solarized Dark, Midnight Blue
- Theme selector dropdown in header
- Theme persists across sessions via localStorage
- Auto-detects OS light/dark preference for first-time visitors
- Listens for OS theme changes in real-time
- All ~50 hardcoded RGBA tint values converted to CSS custom properties for full theme coverage
- No-transition class prevents flash on page load

### Bug Fixes
- **BUG-002:** Fixed job logs route ordering that caused intermittent "repo not found" errors when viewing logs
- **BUG-003:** Fixed duplicate `filterBranches` method — dispatch modal branch filter now uses `filterDispatchBranches`

---

## v2.2 (2026-02-20)

### Inline Job Logs
- View GitHub Actions job logs directly in the dashboard without leaving the page
- ANSI escape code stripping for clean log display
- Step-level log parsing via `##[group]`/`##[endgroup]` markers
- Timestamp and directive prefix stripping (`##[debug]`, `##[warning]`, `##[error]`, `##[command]`)
- 2MB log truncation with indicator to prevent memory issues
- Client-side log caching with automatic re-fetch for in-progress jobs
- "View Logs" button on expanded jobs in both Active Runs and History tabs
- "Details" button on History tab rows for inline run expansion
- Handles expired logs (410 Gone) gracefully

### Comparison Document
- Added [Comparison.md](Comparison.md) documenting alternatives in the GitHub Actions monitoring space

---

## v2.1 (2026-02-20)

### Branches Tab Disabled
- Branches tab and all branch API routes temporarily disabled (will revisit later)
- Code preserved with `#[allow(dead_code)]` for future re-enablement

### Branch Scanning Optimization
- Concurrent batch processing (10 at a time) for commit date lookups
- Only check merge status for branches older than 30 days

### Sidebar Repo Selection
- Click sidebar repos to toggle selection (accent border highlight)
- Filters which repos are scanned in Branches tab
- Count display shows "N of total selected"

---

## v2.0 (2026-02-20)

### Security Hardening
- Bearer token authentication for all API endpoints (`--dashboard-token` / `DASHBOARD_TOKEN`)
- CSRF protection via `X-Requested-With` header validation
- Rate limiting on API endpoints (~60 requests/min per IP via `actix-governor`)
- CORS lockdown: explicit origin allowlist (`--cors-origins`)
- Security headers: `Content-Security-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy`
- XSS prevention: `escJs()` for JavaScript contexts, `safeOpenUrl()` for URL validation
- Error message sanitization: internal details no longer leaked to HTTP clients
- Sensitive field redaction in `Debug` output
- WebSocket authentication via query param token
- WebSocket connection limit (max 50 concurrent)
- Maximum monitored repos cap (`--max-repos`, default 50)
- Branch name validation with safe-character enforcement
- ETag cache size cap (max 1000 entries with 10% eviction)
- Frontend auth dialog with sessionStorage token management

### Branches Tab
- Stale branch detection and cleanup suggestions
- One-click branch deletion for non-protected, non-default branches
- Paginated branch listing (up to 2000 branches per repo)

### Actions Tab & Refresh
- Searchable card grid of all workflows
- One-click Run button + Configure & Run for custom inputs
- Manual refresh button in header

### Workflow Input Discovery
- Auto-discovers `workflow_dispatch` inputs from workflow YAML
- Typed input forms: string, boolean (checkbox), choice (dropdown), number, environment

### Search, Filters & What's Happening
- Search bars on Active Runs, Workflows, Actions, and History tabs
- Time range filters on History tab (1h, 6h, 12h, 3d, 7d, custom)
- What's Happening tab with activity stats, per-repo breakdowns, and timeline
- Smart branch sorting: main/master/develop listed first

### Repo Search & Bulk Add
- Search GitHub repos by name, org, or topic
- Bulk-select and add multiple repos from search results

### macOS DMG Installer
- One-command DMG build with `.app` bundle
- Universal binary support (Intel + Apple Silicon)
- Install/Uninstall scripts included
- Token prompt on first launch with config persistence

### Bug Fixes
- **BUG-001:** Fixed universal DMG filename to show "universal" instead of host architecture

---

## v1.0 (2026-02-20)

### Initial Release
- Real-time GitHub Actions monitoring dashboard
- Rust (actix-web 4) backend with embedded single-file HTML frontend
- WebSocket-based live updates with auto-reconnect
- Multi-repo support with runtime add/remove
- Active Runs tab with expandable job/step details
- Workflows tab with searchable table
- History tab with completed runs
- Workflow dispatch with branch selection
- Cancel and re-run workflows from the dashboard
- GitHub webhook support (HMAC-SHA256 verified)
- Smart polling with ETag caching and rate limit awareness
- Structured logging with optional JSON output
- Graceful shutdown via CancellationToken
- Docker support with multi-stage build
