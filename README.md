# GitWorkflowDashboard

A real-time GitHub Actions monitoring dashboard — single pane of glass for workflows across multiple repositories.

Built with Rust (actix-web 4 + reqwest) and a modular embedded HTML frontend assembled at compile time. No Node.js, no separate frontend build.

## Highlights

- **Real-time monitoring** — WebSocket live updates + GitHub webhooks
- **6 dashboard tabs** — Active Runs, Workflows, Actions, History, What's Happening, Analytics
- **Pipeline orchestration** — Custom workflow tabs with numbered stages, inter-stage gates, and sequential execution
- **Slack notifications** — Template library with `{{variable}}` substitution, multi-URL webhooks, before/after stage triggers
- **Inline job logs** — View logs directly in the dashboard with ANSI stripping, step grouping, and per-step extraction
- **Workflow dispatch** — Trigger workflows with auto-discovered typed input forms (string, boolean, choice, environment)
- **Collapsible workflow groups** — Grouped by repo with alternating colors and expand/collapse toggle
- **Multi-repo** — Search, bulk-add, and manage repos with SQLite persistence (`~/.gwd/dashboard.db`)
- **Analytics** — Server-side SQL analytics: success rates, durations, triggers, actors, failures
- **Adaptive polling** — Exponential backoff with jitter when GitHub rate limit is low
- **5 themes** — Terminal Dark, Light, Monokai, Solarized Dark, Midnight Blue
- **Security** — Auth, rate limiting, CORS, CSRF, XSS prevention, security headers
- **Cmd+K command bar** — Natural language workflow dispatch with fuzzy search
- **Modular frontend** — 25 source files assembled by `build.rs` at compile time into a single embedded HTML
- **Single binary** — Embedded frontend, Docker, and macOS DMG installer
- **5-minute video intro** — Narrated presentation with live demo screenshots

See [Features.md](Features.md) for the complete feature list.

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
| GET | `/api/runs/active` | All in-progress + queued runs |
| GET | `/api/runs/{owner}/{repo}` | Runs for a repo |
| GET | `/api/runs/{owner}/{repo}/{id}` | Run detail with jobs + steps |
| GET | `/api/runs/{owner}/{repo}/jobs/{job_id}/logs` | Job logs (ANSI-stripped, step-parsed) |
| GET | `/api/runs/{owner}/{repo}/jobs/{job_id}/logs/plain` | Plain-text job logs (250K truncation) |
| POST | `/api/runs/{owner}/{repo}/{id}/cancel` | Cancel a run |
| POST | `/api/runs/{owner}/{repo}/{id}/rerun` | Re-run a workflow |
| GET | `/api/analytics/summary?range=7d` | Pre-computed analytics (SQL) |
| POST | `/api/refresh` | Manual refresh all repos |
| POST | `/api/integrations/slack/message` | Slack webhook relay (HTTPS-only) |
| POST | `/api/webhook` | GitHub webhook receiver |
| GET | `/ws/live` | WebSocket for real-time updates |

## Architecture

```
src/
├── main.rs              — Server bootstrap, shared state, security middleware, graceful shutdown
├── config.rs            — CLI args (clap), validation, env var merging
├── database.rs          — SQLite persistence (Mutex<Connection>, repos, runs, analytics)
├── error.rs             — DashboardError with sanitized HTTP responses
├── models.rs            — GitHub API types, newtype IDs (RunId/JobId/WorkflowId), WS messages
├── repo_store.rs        — Legacy JSON repo persistence (fallback)
├── github/
│   ├── client.rs        — GitHub API client with ETag caching, rate limit tracking
│   ├── log_parser.rs    — ANSI stripping, job log step parsing (##[group]/##[endgroup])
│   ├── poller.rs        — Background polling with adaptive exponential backoff + state diffing
│   └── webhook.rs       — HMAC-SHA256 webhook verification
├── handlers/
│   ├── analytics.rs     — Server-side analytics API (spawn_blocking + SQL)
│   ├── integrations.rs  — Slack webhook relay (POST /api/integrations/slack/message)
│   ├── repos.rs         — Runtime repo management + search
│   ├── workflows.rs     — Workflow listing, dispatch, input discovery
│   ├── runs.rs          — Run management, inline job logs (parsed + plain-text), manual refresh
│   └── ws.rs            — WebSocket handler with auth, connection limits, initial state
├── middleware/
│   └── auth.rs          — Bearer token auth (constant-time) + CSRF protection
└── frontend/            — Modular source files → build.rs assembles into $OUT_DIR/index.html
    ├── mod.rs           — include_str!(concat!(env!("OUT_DIR"), "/index.html"))
    ├── shell_head.html  — DOCTYPE, <head>, meta, <style> opener
    ├── shell_mid.html   — </style>, HTML body markup, <script> opener
    ├── shell_foot.html  — </script></body></html>
    ├── styles/          — 13 CSS files (01-variables through 13-commandbar)
    └── js/              — 9 JS files (01-state through 09-commandbar)
```

## Tests

273 tests (62 lib + 106 binary + 105 integration):

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

## Documentation

| Document | Description |
|----------|-------------|
| [Features.md](Features.md) | Complete feature list organized by category |
| [RELEASES.md](RELEASES.md) | Version history and release notes |
| [Comparison.md](Comparison.md) | How this project compares to alternatives |
| [Bugs.md](Bugs.md) | Bug tracker with root cause analysis |
| [PLAN.md](PLAN.md) | Current and past feature plans |
| [audio-video.md](audio-video.md) | Checklist for producing presentation videos |
| [slides/](slides/) | Presentation slides, demo screenshots, and intro video |
