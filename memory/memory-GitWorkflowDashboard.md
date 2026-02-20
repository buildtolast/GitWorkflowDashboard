# GitWorkflowDashboard

## Overview
- **Location**: ~/GIT/GitWorkflowDashboard
- **Purpose**: GitHub Actions monitoring dashboard — single pane of glass for workflows across multiple repos
- **Stack**: Rust (actix-web 4, reqwest 0.12, tokio 1) + embedded single-file HTML frontend
- **Edition**: 2024
- **Binary name**: `git-workflow-dashboard`

## Running
- Dev: `GITHUB_TOKEN=xxx ./run.sh` (default: port 4000)
- Docker: `GITHUB_TOKEN=xxx ./run.sh --docker`
- DMG: `./make-dmg.sh` or `./run.sh --dmg`
- Stop: `./run.sh --stop`
- Direct: `cargo run -- --github-token xxx --repos owner/repo --port 4000`
- Dashboard URL: http://localhost:4000

## CLI Arguments (clap)
- `--github-token` (required) — GitHub PAT
- `--repos` (optional) — Comma-separated `owner/repo` list
- `--port` (default: 4000) — Server port
- `--host` (default: 127.0.0.1) — Server host
- `--poll-interval` (default: 30) — Seconds between polls (10-300)
- `--webhook-secret` (optional) — For HMAC-SHA256 webhook verification
- `--github-api-url` (default: https://api.github.com) — For GitHub Enterprise

## Environment Variable Overrides
GITHUB_TOKEN, GITHUB_REPOS, SERVER_HOST, SERVER_PORT, POLL_INTERVAL, WEBHOOK_SECRET, GITHUB_API_URL, RUST_LOG, LOG_FORMAT

## Architecture

### Backend Modules
```
src/
├── main.rs              — Server bootstrap, shared state, background poller, graceful shutdown
├── lib.rs               — pub mod config, error, models
├── config.rs            — Args (clap), AppConfig, RepoRef, validate(), parse_repos(), 17 unit tests
├── error.rs             — DashboardError (thiserror + ResponseError, 9 variants), 6 unit tests
├── models.rs            — Workflow, WorkflowRun, WorkflowJob, WorkflowStep, WsMessage, WorkflowInput, etc., 12 unit tests
├── github/
│   ├── mod.rs           — Module declarations, ETagCache type
│   ├── client.rs        — GitHubClient: all API calls, rate limit tracking, ETag caching, workflow input discovery
│   ├── poller.rs        — Background polling task with CancellationToken, state diffing, broadcast, PreviousRunTracker
│   └── webhook.rs       — Webhook handler (HMAC-SHA256 verification), 3 unit tests
├── handlers/
│   ├── mod.rs
│   ├── branches.rs      — GET /api/branches/stale, DELETE /api/branches/{owner}/{repo}/{branch} (stale branch cleanup)
│   ├── repos.rs         — GET/POST/DELETE /api/repos, GET /api/repos/search (runtime repo management + search)
│   ├── workflows.rs     — GET /api/workflows, POST /api/workflows/dispatch, GET /api/branches, GET workflow inputs
│   ├── runs.rs          — GET /api/runs/active, GET run detail, POST cancel/rerun, POST /api/refresh (manual poll)
│   └── ws.rs            — WebSocket /ws/live (broadcast state + run updates)
└── frontend/
    ├── mod.rs           — include_str!("index.html")
    └── index.html       — Single-file HTML+CSS+JS dashboard (~5100 lines)
tests/
└── integration_tests.rs — 37 integration tests
```

### Key Patterns
- **SharedDashboardState**: `Arc<RwLock<DashboardState>>` — repos, workflows, runs, rate limit
- **SharedRepoList**: `Arc<RwLock<Vec<RepoRef>>>` — runtime-mutable via POST/DELETE /api/repos
- **PreviousRunTracker**: `Arc<RwLock<HashMap<u64, String>>>` — shared between poller and manual refresh handler
- **WebSocket**: `broadcast::Sender<WsMessage>(4096)` fan-out to all clients
- **Background Poller**: configurable interval, ETag conditional requests, adaptive rate limiting, always broadcasts full state
- **GitHubClient**: reqwest with ETag cache, rate limit header extraction, auth-method-agnostic design
- **Graceful shutdown**: CancellationToken + tokio::select! between server and ctrl_c
- **Auth**: PAT now, designed for GitHub App support later

## Dashboard Tabs
1. **Active Runs** — Live in_progress + queued runs with expandable job/step details, cancel/rerun buttons
2. **Workflows** — Searchable table of all workflow definitions with search filter, trigger button per workflow
3. **Actions** — Searchable card grid for quick-triggering workflows: one-click Run (default branch), Configure & Run (modal with branch/inputs), latest run status per workflow, GitHub link
4. **History** — Searchable completed runs table with time range filters (1h/6h/12h/3d/7d/custom), re-run button per row
5. **Branches** — Stale/merged branch cleanup across all repos with age filters (All/1mo/3mo/6mo/1yr), summary cards, individual/bulk delete, cleanup suggestions
6. **What's Happening** — Activity overview with time range filters, narrative summary, stat cards, per-repo and per-workflow breakdowns with success rate bars, activity timeline

## Workflow Input Discovery
- **Backend**: `get_workflow_inputs()` in `client.rs` fetches workflow YAML via GitHub Contents API, base64 decodes, parses YAML with serde_yaml to extract `on.workflow_dispatch.inputs`
- **YAML `on` key quirk**: serde_yaml parses bare `on` as boolean `true`, so lookup uses both `yaml.get("on")` and `yaml.get(serde_yaml::Value::Bool(true))`
- **Input types supported**: string (text input), boolean (checkbox), choice (select dropdown with options), number (number input), environment (text input)
- **API endpoint**: `GET /api/workflows/{owner}/{repo}/{workflow_id}/inputs` returns `WorkflowInputsResponse { workflow_id, has_dispatch, inputs }`
- **Frontend**: Dispatch modal fetches branches + inputs concurrently, renders typed form fields with descriptions, validates required fields, shows warning if workflow has no `workflow_dispatch` trigger
- **Quick-run**: Checks for inputs first — if workflow has required inputs, opens modal instead of dispatching directly

## Branch Handling
- **Sorting**: main → master → develop → development first, then alphabetically
- **Search**: Text filter above branch select in dispatch modal filters branches by name
- **Select**: Multi-line scrollable list (size=6) instead of dropdown for easier browsing
- **Pagination**: Fetches up to 2000 branches (100 per page, 20-page safety cap)

## Stale Branch Cleanup (Branches Tab)
- **Backend**: `get_branch_details()` in `client.rs` fetches branches, commit dates, and merge status for each branch
- **Merge detection**: Uses GitHub Compare API (`GET /repos/{o}/{r}/compare/{default}...{branch}`) — if status is "identical" or "behind", branch is merged
- **Commit date**: Uses GitHub Commits API (`GET /repos/{o}/{r}/commits/{sha}`) to get last commit date
- **Default branch**: Uses GitHub Repos API (`GET /repos/{o}/{r}`) to determine default branch
- **Delete**: Uses GitHub Refs API (`DELETE /repos/{o}/{r}/git/refs/heads/{branch}`)
- **safe_to_delete**: `is_merged && !protected && !is_default && age_days > 30`
- **API**: `GET /api/branches/stale` returns `StaleBranchesResponse { total, branches, cleanup_suggestions }`
- **API**: `DELETE /api/branches/{owner}/{repo}/{branch}` deletes a branch
- **Frontend**: On-demand fetch (Refresh button), 5-min cache, search filter, age filter (All/1mo/3mo/6mo/1yr), summary cards (Total Stale/Merged & Safe/Protected/Repos Affected), branch table with checkboxes, individual and bulk delete with confirmation
- **Cleanup suggestions**: Generated server-side based on analysis — merged branch count, very old branches (6+ months), unprotected default branches, auto-delete head branches recommendation, workflow concurrency for multi-repo setups

## API Routes
| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Serve embedded HTML frontend |
| GET | `/api/repos` | List monitored repos |
| GET | `/api/repos/search?q={query}` | Search GitHub repos by name/org/topic |
| POST | `/api/repos` | Add repos to monitoring |
| DELETE | `/api/repos` | Remove repos from monitoring |
| GET | `/api/workflows` | All workflows across all repos |
| GET | `/api/workflows/{owner}/{repo}` | Workflows for one repo |
| GET | `/api/workflows/{owner}/{repo}/{workflow_id}/inputs` | Get workflow_dispatch inputs |
| POST | `/api/workflows/dispatch` | Trigger a workflow_dispatch |
| GET | `/api/branches/stale` | Stale branches across all repos with cleanup suggestions |
| GET | `/api/branches/{owner}/{repo}` | List branches |
| DELETE | `/api/branches/{owner}/{repo}/{branch}` | Delete a branch |
| GET | `/api/runs/active` | All in_progress + queued runs |
| GET | `/api/runs/{owner}/{repo}` | Runs for a repo |
| GET | `/api/runs/{owner}/{repo}/{run_id}` | Run detail with jobs + steps |
| POST | `/api/runs/{owner}/{repo}/{run_id}/cancel` | Cancel a run |
| POST | `/api/runs/{owner}/{repo}/{run_id}/rerun` | Re-run a workflow |
| POST | `/api/refresh` | Manual refresh — poll all repos immediately |
| POST | `/api/webhook` | GitHub webhook receiver |
| GET | `/ws/live` | WebSocket for real-time updates |

## Tests (75 total)
- `config::tests` (17): defaults, validation, repo parsing, display
- `error::tests` (6): display strings, HTTP status codes
- `models::tests` (12): serde roundtrips, tagged enum variants, optional fields
- `github::webhook::tests` (3): HMAC signature verification
- Integration tests (37): config validation, repo validation, model serialization, error types, status codes

## Docker
- **Dockerfile**: 2-stage (rust:1.88-bookworm → debian:bookworm-slim), no cmake/libssl needed (reqwest uses rustls)
- **docker-compose.yml**: App-only (no infrastructure dependencies)

## macOS DMG Installer
- **Build**: `./make-dmg.sh` or `./run.sh --dmg` — builds release binary, .app bundle, compressed DMG
- **Universal**: `./make-dmg.sh --universal` — builds x86_64 + arm64 lipo universal binary
- **Clean**: `./make-dmg.sh --clean` — removes build artifacts
- **Output**: `GitWorkflowDashboard-v{version}-{arch}.dmg` (~4.7MB for x86_64)
- **App bundle**: `GitWorkflowDashboard.app` with launcher script, auto-opens browser, saves token to `~/.config/git-workflow-dashboard/config.env`
- **First launch**: macOS dialog prompts for GitHub PAT, saves config with 600 permissions
- **Files**: `make-dmg.sh` (orchestrator), `installer/macos/` (Info.plist, launcher.sh, create-icon.sh)
- **Icon**: Generated programmatically via Python (git branch/node/checkmark design on dark terminal background), converted to .icns via sips + iconutil

## Frontend Features
- Header: WebSocket status indicator, rate limit badge, last poll time, manual Refresh button (spinning icon)
- Sidebar: Repo list with active-run indicators, add/remove repos, resizable via drag handle, repo search with bulk add
- Actions tab: searchable workflow cards with quick-run (auto-detects main/master branch), configure & run (dispatch modal with branch selector + typed inputs), latest run status
- History tab: searchable table with time range filters (All/1h/6h/12h/3d/7d/custom datetime range), re-run button per completed run
- Branches tab: stale/merged branch cleanup across all repos, search + age filters (All/1mo/3mo/6mo/1yr), summary cards, branch table with status badges (merged/stale/protected/default/safe), individual + bulk delete, cleanup suggestions section
- Dispatch modal: branch search filter, sorted branches (main/master/develop first), fetches workflow inputs concurrently, renders typed fields (string→text, boolean→checkbox, choice→select, number→number), validates required fields, warns if no workflow_dispatch trigger
- What's Happening tab: narrative summary, stat cards (total/succeeded/failed/cancelled/in-progress/success-rate), per-repo and per-workflow breakdowns with stacked progress bars, activity timeline with color-coded events
- Toast notifications for all operations
- Scanline overlay toggle (retro CRT effect)
- Tab state persisted in localStorage

## Repo Search & Bulk Add
- **Backend**: `search_repos()` in `client.rs` uses GitHub Search API (`GET /search/repositories?q={query}&per_page=20&sort=updated`)
- **Handler**: `GET /api/repos/search?q={query}` — searches GitHub, filters out already-monitored repos
- **Frontend**: Sidebar has Search/Manual toggle tabs — Search mode has debounced (400ms) input, dropdown with checkboxes, bulk "Add Selected" button
- **Query examples**: `kafka`, `org:myorg`, `user:myuser react`, `myorg/` (prefix)
- Search API uses separate 30 req/min rate limit from core API

## Dependencies
actix-web 4, actix-ws 0.3, actix-rt 2, actix-cors 0.7, reqwest 0.12 (rustls-tls), tokio 1, tokio-util 0.7, futures-util 0.3, serde 1, serde_json 1, serde_yaml 0.9, base64 0.22, urlencoding 2, chrono 0.4, clap 4, thiserror 2, tracing 0.1, tracing-subscriber 0.3, tracing-actix-web 0.7, hmac 0.12, sha2 0.10, hex 0.4, log 0.4

## Notes
- Workflows must have `workflow_dispatch:` in their `on:` trigger block to be triggerable from the dashboard
- The poller broadcasts full `StateUpdate` every cycle (not just run diffs) so workflows always stay fresh for all clients
- GitHub PAT needs `actions:read` scope (or `repo` for private repos); `actions:write` for dispatch/cancel/rerun; `repo` scope needed for branch deletion
- Workflow input discovery uses GitHub Contents API to fetch YAML, not a dedicated inputs endpoint
- Branch sorting prioritizes main/master/develop/development, then alphabetical
