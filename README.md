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
- **Cancel / Re-run** — Control running workflows directly from the dashboard
- **Runtime repo management** — Add/remove repos without restarting
- **Manual refresh** — On-demand polling via header button or `POST /api/refresh`
- **Smart polling** — ETag conditional requests (304s don't count against rate limit), adaptive interval
- **GitHub webhook support** — HMAC-SHA256 verified webhook receiver for instant updates
- **Rate limit awareness** — Live rate limit badge, auto-skip polls when limit is critically low
- **Structured logging** — tracing with optional JSON output (`LOG_FORMAT=json`)
- **Graceful shutdown** — CancellationToken-based clean shutdown on SIGINT
- **Docker support** — Multi-stage build, single binary runtime

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

### Environment Variables

All CLI arguments can be set via environment variables:

`GITHUB_TOKEN`, `GITHUB_REPOS`, `SERVER_HOST`, `SERVER_PORT`, `POLL_INTERVAL`, `WEBHOOK_SECRET`, `GITHUB_API_URL`, `RUST_LOG`, `LOG_FORMAT`

## API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Dashboard UI |
| GET | `/api/repos` | List monitored repos |
| POST | `/api/repos` | Add repos `{"repos": ["owner/repo"]}` |
| DELETE | `/api/repos` | Remove repos `{"repos": ["owner/repo"]}` |
| GET | `/api/workflows` | All workflows across repos |
| GET | `/api/workflows/{owner}/{repo}` | Workflows for one repo |
| GET | `/api/workflows/{owner}/{repo}/{id}/inputs` | Get workflow_dispatch inputs |
| POST | `/api/workflows/dispatch` | Trigger a workflow |
| GET | `/api/branches/{owner}/{repo}` | List branches |
| GET | `/api/runs/active` | All in-progress + queued runs |
| GET | `/api/runs/{owner}/{repo}` | Runs for a repo |
| GET | `/api/runs/{owner}/{repo}/{id}` | Run detail with jobs + steps |
| POST | `/api/runs/{owner}/{repo}/{id}/cancel` | Cancel a run |
| POST | `/api/runs/{owner}/{repo}/{id}/rerun` | Re-run a workflow |
| POST | `/api/refresh` | Manual refresh all repos |
| POST | `/api/webhook` | GitHub webhook receiver |
| GET | `/ws/live` | WebSocket for real-time updates |

## Architecture

```
src/
├── main.rs              — Server bootstrap, shared state, graceful shutdown
├── config.rs            — CLI args (clap), validation, env var merging
├── error.rs             — DashboardError with HTTP status mapping
├── models.rs            — GitHub API types, WorkflowInput types, WebSocket message types
├── github/
│   ├── client.rs        — GitHub API client with ETag caching, rate limit tracking, workflow input discovery
│   ├── poller.rs        — Background polling task with state diffing + broadcast
│   └── webhook.rs       — HMAC-SHA256 webhook verification
├── handlers/
│   ├── repos.rs         — Runtime repo management
│   ├── workflows.rs     — Workflow listing, dispatch, input discovery
│   ├── runs.rs          — Run management + manual refresh
│   └── ws.rs            — WebSocket handler with initial state snapshot
└── frontend/
    └── index.html       — Embedded single-file dashboard (~4000 lines)
```

## Tests

75 tests (38 unit + 37 integration):

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

## run.sh

```bash
./run.sh                # Run from source with cargo
./run.sh --docker       # Run in Docker
./run.sh --stop         # Stop whatever was started
./run.sh --help         # Show help
```
