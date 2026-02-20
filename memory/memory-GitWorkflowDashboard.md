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
├── models.rs            — Workflow, WorkflowRun, WorkflowJob, WorkflowStep, WsMessage, DashboardState, etc., 12 unit tests
├── github/
│   ├── mod.rs           — Module declarations, ETagCache type
│   ├── client.rs        — GitHubClient: all API calls, rate limit tracking, ETag caching
│   ├── poller.rs        — Background polling task with CancellationToken, state diffing, broadcast
│   └── webhook.rs       — Webhook handler (HMAC-SHA256 verification), 3 unit tests
├── handlers/
│   ├── mod.rs
│   ├── repos.rs         — GET/POST/DELETE /api/repos (runtime repo management)
│   ├── workflows.rs     — GET /api/workflows, POST /api/workflows/dispatch, GET /api/branches
│   ├── runs.rs          — GET /api/runs/active, GET run detail, POST cancel/rerun
│   └── ws.rs            — WebSocket /ws/live (broadcast state + run updates)
└── frontend/
    ├── mod.rs           — include_str!("index.html")
    └── index.html       — Single-file HTML+CSS+JS dashboard
tests/
└── integration_tests.rs — 37 integration tests
```

### Key Patterns
- **SharedDashboardState**: `Arc<RwLock<DashboardState>>` — repos, workflows, runs, rate limit
- **SharedRepoList**: `Arc<RwLock<Vec<RepoRef>>>` — runtime-mutable via POST/DELETE /api/repos
- **WebSocket**: `broadcast::Sender<WsMessage>(4096)` fan-out to all clients
- **Background Poller**: configurable interval, ETag conditional requests, adaptive rate limiting
- **GitHubClient**: reqwest with ETag cache, rate limit header extraction, auth-method-agnostic design
- **Graceful shutdown**: CancellationToken + tokio::select! between server and ctrl_c
- **Auth**: PAT now, designed for GitHub App support later

## API Routes
| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Serve embedded HTML frontend |
| GET | `/api/repos` | List monitored repos |
| POST | `/api/repos` | Add repos to monitoring |
| DELETE | `/api/repos` | Remove repos from monitoring |
| GET | `/api/workflows` | All workflows across all repos |
| GET | `/api/workflows/{owner}/{repo}` | Workflows for one repo |
| POST | `/api/workflows/dispatch` | Trigger a workflow_dispatch |
| GET | `/api/branches/{owner}/{repo}` | List branches |
| GET | `/api/runs/active` | All in_progress + queued runs |
| GET | `/api/runs/{owner}/{repo}` | Runs for a repo |
| GET | `/api/runs/{owner}/{repo}/{run_id}` | Run detail with jobs + steps |
| POST | `/api/runs/{owner}/{repo}/{run_id}/cancel` | Cancel a run |
| POST | `/api/runs/{owner}/{repo}/{run_id}/rerun` | Re-run a workflow |
| POST | `/api/webhook` | GitHub webhook receiver |
| GET | `/ws/live` | WebSocket for real-time updates |

## Tests (75 total)
- `config::tests` (17): defaults, validation, repo parsing, display
- `error::tests` (6): display strings, HTTP status codes
- `models::tests` (12): serde roundtrips, tagged enum variants, optional fields
- `github::webhook::tests` (3): HMAC signature verification
- Integration tests (37): config validation, repo validation, model serialization, error types, status codes

## Docker
- **Dockerfile**: 2-stage (rust:1.88-bookworm → debian:bookworm-slim), no cmake/libssl needed
- **docker-compose.yml**: App-only (no infrastructure dependencies)

## Dependencies
actix-web 4, actix-ws 0.3, actix-rt 2, actix-cors 0.7, reqwest 0.12 (rustls-tls), tokio 1, tokio-util 0.7, futures-util 0.3, serde 1, serde_json 1, chrono 0.4, clap 4, thiserror 2, tracing 0.1, tracing-subscriber 0.3, tracing-actix-web 0.7, hmac 0.12, sha2 0.10, hex 0.4, log 0.4
