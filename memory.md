# GitWorkflowDashboard Memory

## Purpose
Persistent project memory for the dashboard architecture, runtime behavior, and operational decisions.

## Runtime Sources of Truth
- App startup config (macOS app launcher):
  - `~/.config/git-workflow-dashboard/config.env`
- Frontend persisted state (browser localStorage):
  - `gwd_custom_workflow_tabs`
  - `gwd_pipeline_presets`
  - `gwd_saved_repos`
  - `gwd_slack_templates`
  - `gwd_slack_config`

## Core Stack
- Backend: Rust, `actix-web`, `reqwest`, `tokio`
- Frontend: embedded single-file HTML/CSS/JS (`src/frontend/index.html`)
- Realtime: WebSocket state + run update broadcast

## Current Product Capabilities
- Multi-repo workflow monitoring and dispatch
- Custom workflow tabs with stage orchestration
- Parallel execution within stage, sequential stages
- Manual approval inter-stage gates
- Slack stage steps (before/after boundaries)
- Slack template library + multi-webhook delivery
- Agent/bot with periodic + critical override notifications
- Per-pipeline branch and workflow input configuration
- Import/export for plans, presets, repos
- On-demand logs and expanded log viewing
- Failure recovery with:
  - Resume from failed step
  - Start over from beginning

## Design Notes
- Keep plan state backward-compatible when possible.
- Prefer non-breaking storage key evolution.
- Validate flow changes with `cargo test` and JS parse checks.

## Linked Detailed Docs
- `memory/memory-GitWorkflowDashboard.md`
- `memory/context-GitWorkflowDashboard.md`
- `memory/plan-GitWorkflowDashboard.md`
