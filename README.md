# GitWorkflowDashboard

A real-time GitHub Actions monitoring dashboard — single pane of glass for workflows across multiple repositories.

Native desktop application built with Rust + [Slint](https://slint.dev) UI framework. Background polling via tokio async runtime.

## Highlights

- **Native desktop app** — Slint Rust GUI, no browser required
- **Real-time monitoring** — Background poller with broadcast channel updates
- **6 dashboard tabs** — Active Runs, Workflows, Actions, History, What's Happening, Analytics
- **Inline job logs** — View logs directly in the dashboard with ANSI stripping and step grouping
- **Workflow dispatch** — Trigger workflows with auto-discovered typed input forms
- **Multi-repo** — Search, bulk-add, and manage repos at runtime
- **5 themes** — Terminal Dark, Light, Monokai, Solarized Dark, Midnight Blue (file-persisted)
- **Single binary** — Everything compiled in, Docker build available for CI

See [Features.md](Features.md) for the complete feature list.

## Quick Start

```bash
# From source
GITHUB_TOKEN=ghp_xxx GITHUB_REPOS=owner/repo cargo run

# With run.sh
GITHUB_TOKEN=ghp_xxx GITHUB_REPOS=owner/repo ./run.sh

# Docker (CI/build only — native GUI requires display)
GITHUB_TOKEN=ghp_xxx docker compose up -d --build

# Direct with options
cargo run -- --github-token ghp_xxx --repos owner/repo1,owner/repo2
```

## Requirements

- **Rust 1.90+** (Slint dependency requirement)
- **GitHub PAT** with `actions:read` scope (or `repo` for private repos)
- For dispatch/cancel/rerun: `actions:write` scope
- Workflows must include `workflow_dispatch:` in their `on:` block to be triggerable

## Configuration

### CLI Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `--github-token` | (required) | GitHub Personal Access Token |
| `--repos` | (none) | Comma-separated `owner/repo` list |
| `--poll-interval` | 30 | Seconds between polls (10-300) |
| `--github-api-url` | https://api.github.com | For GitHub Enterprise |
| `--max-repos` | 50 | Maximum number of monitored repos |

### Environment Variables

All CLI arguments can be set via environment variables:

`GITHUB_TOKEN`, `GITHUB_REPOS`, `POLL_INTERVAL`, `GITHUB_API_URL`, `MAX_REPOS`, `RUST_LOG`, `LOG_FORMAT`

## Architecture

```
src/
├── main.rs              — Slint UI entry point, callback wiring, command channel
├── lib.rs               — Library root: modules + slint::include_modules!()
├── config.rs            — CLI args (clap), validation, env var merging
├── error.rs             — DashboardError enum (actix ResponseError behind "web" feature)
├── models.rs            — GitHub API types, WebSocket message types
├── repo_store.rs        — Persistent repo list (JSON file)
├── github/
│   ├── client.rs        — GitHub API client with ETag caching, rate limit tracking
│   ├── poller.rs        — Background polling task with state diffing + broadcast
│   └── webhook.rs       — HMAC-SHA256 webhook verification (behind "web" feature)
└── ui/
    ├── mod.rs           — UI module root
    ├── backend.rs       — Tokio runtime bridge: poller, command handler, state→UI push
    ├── actions.rs       — UiCommand handlers (add/remove repos, dispatch, cancel, etc.)
    ├── bridge.rs        — Rust model → Slint model type conversions
    └── theme.rs         — 5 theme palettes with file persistence (~/.gwd/theme)

ui/                      — Slint markup files
├── main.slint           — Root window: layout, property declarations, exports
├── theme.slint          — ThemeColors struct + Theme global with defaults
├── widgets.slint        — Reusable: StatusDot, StatusBadge, ActionButton, SearchInput, etc.
├── header.slint         — Top bar: logo, connection status, rate limit, theme selector
├── sidebar.slint        — Repo list, search, add/remove
├── tab_bar.slint        — Tab navigation with active indicator
├── active_runs.slint    — Active Runs tab with expandable job/step details
├── workflows.slint      — Workflows tab: searchable table
├── actions_tab.slint    — Actions tab: quick run + configure dispatch
├── history.slint        — History tab: time filters, expandable details
├── whats_happening.slint — Activity overview with aggregate stats
├── analytics.slint      — Analytics: actors, success rates, duration analysis
├── dispatch_dialog.slint — Workflow dispatch modal with typed inputs
├── log_viewer.slint     — Collapsible bottom panel for job logs
├── toast.slint          — Toast notification overlay
└── branches.slint       — Branches tab (disabled stub)
```

## Tests

160 tests (60 lib + 100 integration):

```bash
cargo test
```

Note: Tests using `actix_web::ResponseError` are behind `#[cfg(feature = "web")]` and skipped by default.

## Docker

```bash
# Build and run (primarily for CI — native GUI needs a display server)
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

# Build universal binary (Intel + Apple Silicon)
./make-dmg.sh --universal
```

## run.sh

```bash
./run.sh                # Run from source with cargo
./run.sh --docker       # Run in Docker
./run.sh --dmg          # Build macOS DMG installer
./run.sh --build        # Clean, test, and build
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
