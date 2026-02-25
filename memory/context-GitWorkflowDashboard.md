# GitWorkflowDashboard Context

## Scope
This document captures the current operating context for custom workflow tab orchestration, persistence, Slack integrations, and plan execution runtime behavior in the embedded frontend (`src/frontend/index.html`).

## Core UI/Runtime Context
- The dashboard uses built-in tabs (`active-runs`, `workflows`, `actions`, `history`, `whats-happening`) plus dynamic custom workflow tabs (`custom-workflow-*`).
- Custom tabs model user-defined deployment plans made from selected workflows across monitored repos.
- Stage model:
  - Same stage number = parallel execution.
  - Next stage executes only after current stage completes.
  - Manual approvals can gate transitions between stages.

## Plan Execution Model
- `App.runActiveWorkflowTabPlan(opts)` drives execution.
- Failure-handling supports restart options:
  - **Resume from Failed Step**: uses an in-memory `resumeStep` cursor.
  - **Start Over**: clears `resumeStep` and executes from stage 1.
- Resume cursor types:
  - `before_slack` (retry pre-stage Slack step)
  - `stage` (retry failed pipelines in stage)
  - `after_slack` (retry post-stage Slack step)
- Manual gate flow:
  - Runtime sets `awaitingManual` after gated stage.
  - `Continue Plan` resolves the gate and continues execution.

## Slack Context
- Global Slack config accepts multiple webhook URLs.
- Template library supports create/update/delete and per-stage assignment.
- Slack steps can be attached:
  - Before first stage
  - Between stages (before/after)
  - After last stage
- Plan agent composes formatted status messages and posts to all configured webhooks.

## Agent/Bot Context
- Agent can be enabled per custom tab.
- Runtime controls include:
  - Toggle at run time
  - Frequency updates at run time
  - Silence/unsilence at run time
  - Remove agent at run time
- Forced notifications override frequency for critical events (manual gate required, pipeline failure, etc.).

## Persistence and Portability Context
Frontend localStorage keys:
- `gwd_custom_workflow_tabs`
- `gwd_pipeline_presets`
- `gwd_saved_repos`
- `gwd_slack_templates`
- `gwd_slack_config`

JSON portability:
- Export/import full plan state (tabs + presets + templates + repos)
- Export/import variable presets
- Export/import repos

## Observability Context
- Active run rows can fetch and render step/job logs on-demand.
- Log viewer supports expanded reading mode.

## Current UX Context
- Dashboard UI has a refreshed visual system and denser operational controls for plan building.
- Workflows tab uses a dedicated controls shell for tab creation, bulk add-to-tab, and import/export actions.
