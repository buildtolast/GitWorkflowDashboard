# GitWorkflowDashboard

A real-time GitHub Actions monitoring dashboard — single pane of glass for workflows across multiple repositories.

Built with Rust (actix-web 4 + reqwest) and an embedded single-file HTML frontend. No Node.js, no separate frontend build.

## Features

- **Real-time monitoring** — WebSocket-based live updates for all workflow runs
- **Multi-repo** — Monitor workflows across multiple repositories simultaneously
- **5 dashboard tabs:**
  - **Active Runs** — Live in-progress and queued runs with expandable job/step details
  - **Workflows** — Searchable table of all workflow definitions
  - **Actions** — Searchable card grid for quick-triggering workflows with one-click Run or Configure & Run
  - **History** — Searchable completed runs with time range filters (1h/6h/12h/3d/7d/custom) and re-run button
  - **What's Happening** — Activity overview with stats, per-repo/workflow breakdowns, and timeline
- **Workflow dispatch** — Trigger `workflow_dispatch` workflows with searchable branch selector and typed input forms
- **Workflow input discovery** — Auto-discovers `workflow_dispatch` inputs from workflow YAML (string, boolean, choice, number types)
- **Smart branch sorting** — main/master/develop/development branches listed first, with search filter
- **Inline job logs** — View GitHub Actions job logs directly in the dashboard with ANSI stripping and step-level grouping
- **Cancel / Re-run** — Control running workflows directly from the dashboard
- **Repo search & bulk add** — Search GitHub repos by name, org, or topic and add multiple repos at once
- **Runtime repo management** — Add/remove repos without restarting
- **Manual refresh** — On-demand polling via header button or `POST /api/refresh`
- **Smart polling** — ETag conditional requests (304s don't count against rate limit), adaptive interval
- **GitHub webhook support** — HMAC-SHA256 verified webhook receiver for instant updates
- **Rate limit awareness** — Live rate limit badge, auto-skip polls when limit is critically low
- **Structured logging** — tracing with optional JSON output (`LOG_FORMAT=json`)
- **Graceful shutdown** — CancellationToken-based clean shutdown on SIGINT
- **Security hardening** — Bearer token auth, rate limiting, CORS lockdown, CSRF protection, XSS prevention, security headers
- **Docker support** — Multi-stage build, single binary runtime
- **macOS DMG installer** — One-command DMG build with .app bundle, drag-to-install, auto-launch

## Quick Start

```bash
# From source
GITHUB_TOKEN=ghp_xxx GITHUB_REPOS=owner/repo ./run.sh

# Docker
GITHUB_TOKEN=ghp_xxx GITHUB_REPOS=owner/repo ./run.sh --docker

# Direct
cargo run -- --github-token ghp_xxx --repos owner/repo1,owner/repo2
```

Dashboard at **http://localhost:4000**

## Requirements

- **GitHub PAT** with `actions:read` scope (or `repo` for private repos)
- For dispatch/cancel/rerun: `actions:write` scope
- Workflows must include `workflow_dispatch:` in their `on:` block to be triggerable from the dashboard

## Configuration

### CLI Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `--github-token` | (required) | GitHub Personal Access Token |
| `--repos` | (none) | Comma-separated `owner/repo` list |
| `--port` | 4000 | Server port |
| `--host` | 127.0.0.1 | Server host |
| `--poll-interval` | 30 | Seconds between polls (10-300) |
| `--webhook-secret` | (none) | For HMAC-SHA256 webhook verification |
| `--github-api-url` | https://api.github.com | For GitHub Enterprise |
| `--dashboard-token` | (none) | Bearer token to protect API endpoints |
| `--cors-origins` | (none) | Additional CORS origins (comma-separated) |
| `--max-repos` | 50 | Maximum number of monitored repos |

### Environment Variables

All CLI arguments can be set via environment variables:

`GITHUB_TOKEN`, `GITHUB_REPOS`, `SERVER_HOST`, `SERVER_PORT`, `POLL_INTERVAL`, `WEBHOOK_SECRET`, `GITHUB_API_URL`, `DASHBOARD_TOKEN`, `CORS_ORIGINS`, `MAX_REPOS`, `RUST_LOG`, `LOG_FORMAT`

## API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Dashboard UI |
| GET | `/api/repos` | List monitored repos |
| GET | `/api/repos/search?q={query}` | Search GitHub repos |
| POST | `/api/repos` | Add repos `{"repos": ["owner/repo"]}` |
| DELETE | `/api/repos` | Remove repos `{"repos": ["owner/repo"]}` |
| GET | `/api/workflows` | All workflows across repos |
| GET | `/api/workflows/{owner}/{repo}` | Workflows for one repo |
| GET | `/api/workflows/{owner}/{repo}/{id}/inputs` | Get workflow_dispatch inputs |
| POST | `/api/workflows/dispatch` | Trigger a workflow |
| ~~GET~~ | ~~`/api/branches/stale`~~ | ~~Stale branches (disabled)~~ |
| ~~GET~~ | ~~`/api/branches/{owner}/{repo}`~~ | ~~List branches (disabled)~~ |
| ~~DELETE~~ | ~~`/api/branches/{owner}/{repo}/{branch}`~~ | ~~Delete a branch (disabled)~~ |
| GET | `/api/runs/active` | All in-progress + queued runs |
| GET | `/api/runs/{owner}/{repo}` | Runs for a repo |
| GET | `/api/runs/{owner}/{repo}/{id}` | Run detail with jobs + steps |
| GET | `/api/runs/{owner}/{repo}/jobs/{job_id}/logs` | Job logs (ANSI-stripped, step-parsed) |
| POST | `/api/runs/{owner}/{repo}/{id}/cancel` | Cancel a run |
| POST | `/api/runs/{owner}/{repo}/{id}/rerun` | Re-run a workflow |
| POST | `/api/refresh` | Manual refresh all repos |
| POST | `/api/webhook` | GitHub webhook receiver |
| GET | `/ws/live` | WebSocket for real-time updates |

## Architecture

```
src/
├── main.rs              — Server bootstrap, shared state, security middleware, graceful shutdown
├── config.rs            — CLI args (clap), validation, env var merging
├── error.rs             — DashboardError with sanitized HTTP responses
├── models.rs            — GitHub API types, WorkflowInput types, WebSocket message types
├── github/
│   ├── client.rs        — GitHub API client with ETag caching, rate limit tracking, workflow input discovery, job log fetching
│   ├── poller.rs        — Background polling task with state diffing + broadcast
│   └── webhook.rs       — HMAC-SHA256 webhook verification
├── handlers/
│   ├── repos.rs         — Runtime repo management + search
│   ├── workflows.rs     — Workflow listing, dispatch, input discovery
│   ├── branches.rs      — Stale branch detection, cleanup suggestions, deletion
│   ├── runs.rs          — Run management, inline job logs, manual refresh
│   └── ws.rs            — WebSocket handler with auth, connection limits, initial state
├── middleware/
│   └── auth.rs          — Bearer token authentication + CSRF protection
└── frontend/
    └── index.html       — Embedded single-file dashboard (~5000 lines)
```

## Tests

120 tests (36 lib + 47 binary + 37 integration):

```bash
cargo test
```

## Docker

```bash
# Build and run
GITHUB_TOKEN=ghp_xxx docker compose up -d --build

# View logs
docker logs -f git-workflow-dashboard

# Stop
docker compose down
```

## macOS DMG Installer

```bash
# Build DMG for current architecture
./make-dmg.sh

# Or via run.sh
./run.sh --dmg

# Build universal binary (Intel + Apple Silicon)
./make-dmg.sh --universal

# Clean build artifacts
./make-dmg.sh --clean
```

The DMG contains a `GitWorkflowDashboard.app` bundle that:
- Prompts for your GitHub token on first launch (saves to `~/.config/git-workflow-dashboard/config.env`)
- Starts the dashboard server automatically
- Opens your browser to the dashboard
- Runs as a standard macOS app (visible in Dock)

## run.sh

```bash
./run.sh                # Run from source with cargo
./run.sh --docker       # Run in Docker
./run.sh --dmg          # Build macOS DMG installer
./run.sh --build        # Clean, test, and build DMG
./run.sh --clean        # Remove all build artifacts
./run.sh --stop         # Stop whatever was started
./run.sh --help         # Show help
```

## Changelog

### v2.2 (2026-02-20)

#### Inline Job Logs
- View GitHub Actions job logs directly in the dashboard without leaving the page
- ANSI escape code stripping for clean log display
- Step-level log parsing via `##[group]`/`##[endgroup]` markers
- Timestamp and directive prefix stripping (`##[debug]`, `##[warning]`, `##[error]`, `##[command]`)
- 2MB log truncation with indicator to prevent memory issues
- Client-side log caching with automatic re-fetch for in-progress jobs
- "View Logs" button on expanded jobs in both Active Runs and History tabs
- "Details" button on History tab rows for inline run expansion
- Handles expired logs (410 Gone) gracefully
- See [Comparison.md](Comparison.md) for how this project compares to alternatives

### v2.1 (2026-02-20)

#### Branches Tab Disabled
- Branches tab and all branch API routes temporarily disabled (will revisit later)
- Code preserved with `#[allow(dead_code)]` for future re-enablement

### v2 (2026-02-20)

#### Security Hardening
- Bearer token authentication for all API endpoints (`--dashboard-token` / `DASHBOARD_TOKEN`)
- CSRF protection via `X-Requested-With` header validation on API requests
- Rate limiting on API endpoints (~60 requests/min per IP via `actix-governor`)
- CORS lockdown: explicit origin allowlist replacing `allow_any_origin` (`--cors-origins`)
- Security headers: `Content-Security-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy`
- XSS prevention: `escJs()` for JavaScript contexts in onclick handlers, `safeOpenUrl()` for URL validation
- Error message sanitization: internal error details no longer leaked to HTTP clients
- Sensitive field redaction in `Debug` output (`github_token`, `webhook_secret`, `dashboard_token`)
- WebSocket authentication via query param token
- WebSocket connection limit (max 50 concurrent connections)
- Maximum monitored repos cap (`--max-repos`, default 50)
- Branch name validation with safe-character enforcement
- ETag cache size cap (max 1000 entries with 10% eviction)
- Frontend auth dialog with sessionStorage token management and auto-prompt on 401/403

#### Branches Tab
- New **Branches** tab for stale branch detection and cleanup
- Stale branch analysis: age, merge status, protection status, default branch detection
- Cleanup suggestions with impact levels (merged branches, 6+ month old, etc.)
- One-click branch deletion for non-protected, non-default branches
- Paginated branch listing fetching up to 2000 branches per repo

#### Actions Tab & Refresh
- New **Actions** tab with searchable card grid of all workflows
- One-click **Run** button (triggers on default branch) and **Configure & Run** for custom inputs
- Manual refresh button in header for on-demand polling

#### Workflow Input Discovery
- Auto-discovers `workflow_dispatch` inputs by parsing workflow YAML files via GitHub Contents API
- Typed input forms: string, boolean (checkbox), choice (dropdown), number, environment
- Input validation with descriptions, required flags, and default values

#### Search, Filters & What's Happening
- Search bars on Active Runs, Workflows, Actions, and History tabs
- Time range filters on History tab (1h, 6h, 12h, 3d, 7d, custom date picker)
- **What's Happening** tab with activity stats, per-repo breakdowns, and run timeline
- Smart branch sorting: main/master/develop listed first in selectors

#### Repo Search & Bulk Add
- Search GitHub repos by name, org, or topic via GitHub Search API
- Bulk-select and add multiple repos at once from search results

#### macOS DMG Installer
- One-command DMG build with `.app` bundle (`./make-dmg.sh` or `./run.sh --dmg`)
- Universal binary support (`--universal` for Intel + Apple Silicon)
- Install and Uninstall scripts included in DMG
- Token prompt on first launch with config persistence
- Auto-opens browser to dashboard on launch
- `--build` and `--clean` options for `run.sh`

### v1 (2026-02-20)

- Initial implementation
- Real-time WebSocket-based monitoring of GitHub Actions across multiple repos
- Active Runs and Workflows tabs
- History tab with completed run listing
- Workflow dispatch with branch selector
- Run cancel and re-run
- Runtime repo management (add/remove without restart)
- Background poller with ETag caching and rate limit awareness
- GitHub webhook support with HMAC-SHA256 verification
- Docker multi-stage build
- 75 tests (38 unit + 37 integration)
