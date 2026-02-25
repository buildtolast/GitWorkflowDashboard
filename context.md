# GitWorkflowDashboard Build Context and Requirements

## 1. Product Goal
Build a GitHub Actions dashboard that allows teams to:
- monitor workflows across selected repositories,
- compose custom execution plans from selected pipelines/workflows,
- configure per-pipeline workflow inputs,
- orchestrate stage-based execution (sequential and parallel),
- add manual and Slack-based inter-stage controls,
- run with a runtime notification agent,
- persist/share state across sessions and machines.

This document reflects all stakeholder requirements provided during product iteration.

## 2. Core User Workflow
1. User adds/selects repos to monitor.
2. Dashboard loads workflows from selected repos.
3. User selects workflows and creates a named custom tab (plan).
4. User arranges workflows into stages:
   - same stage = run in parallel,
   - next stage = run after previous stage completes.
5. User configures per-pipeline branch and workflow inputs.
6. User adds inter-stage actions (manual approval / Slack messages).
7. User runs the plan with optional Slack agent/bot.
8. User watches status/logs live, handles manual gates, and responds to failures.
9. User can resume from failed step or start over.
10. User exports/imports plan state, variable presets, and repos to share across machines/users.

## 3. Functional Requirements

### 3.1 Repo Management
- User can add repos manually (`owner/repo`) and via search.
- User can remove repos.
- Repos should persist across sessions (no need to reload manually every app start).
- Repos should be exportable/importable via JSON to reuse across machines/users.
- On startup, app should restore saved repos and recover gracefully if partial failures occur.

### 3.2 Workflows Dashboard
- Show workflows from selected repos.
- Provide search/filter.
- Fix branch dropdown/filter bug where common branches (example: `main`) were missing.
- If repo/workflow lists become blank unexpectedly, app should recover from persisted state and restore visibility.

### 3.3 Custom Tabs (Plan Groups)
- User can select pipelines/workflows and create a separate tab.
- User can name the tab (example: `New dev deployment`).
- User can rename/delete custom tabs.
- User can reorder custom tabs in UI (move left/right / drag behavior).

### 3.4 Pipeline Selection and Movement
- User can add pipelines into existing custom tabs.
- If multiple custom tabs exist, user should choose target tab.
- User can remove pipelines from a custom tab.
- Quick action: move a pipeline from one custom tab to another in one flow.

### 3.5 Stage Orchestration
- User defines stage number per pipeline.
- Execution semantics:
  - stage 1, stage 2, ... run sequentially,
  - within same stage, pipelines run in parallel.
- Example requirement:
  - pipeline 1 then pipeline 2 (sequential),
  - then pipeline 3 and 4 (parallel),
  - then pipeline 5.

### 3.6 Per-Pipeline Configuration (Variables)
- Variables mean workflow inputs from `workflow_dispatch`.
- Must persist per pipeline in custom tabs.
- Config must not be lost after leaving config window/view.
- Provide a variables sidebar listing all pipelines in the tab.
- For each pipeline:
  - branch override,
  - workflow input fields by type (string/boolean/choice/number),
  - saved values reused during plan run.
- Export/import variable presets via JSON for cross-machine reuse.

### 3.7 Inter-Stage Steps
- Between stages, user can add:
  - manual intervention step (requires click to continue),
  - Slack notification step.
- UX should allow adding steps with clear `+` action between stages.
- User must be able to remove steps.
- Must support boundaries:
  - before stage 1,
  - after last stage,
  - between all stage transitions.

### 3.8 Slack Notifications and Templates
- Slack step posts custom message via webhook.
- Support multiple webhook URLs (same message to multiple channels).
- Templates available in variables/config scope.
- User can create, save, pre-store, select templates.
- User can edit/update/delete templates.
- User can remove assigned Slack step from stage boundary.
- Slack message content should be well-formatted, readable, and contextual.
- Do not hardcode secrets in source; webhook URLs are user-configured runtime state.

### 3.9 Agent/Bot Requirements
- Agent can be toggled on/off for a plan run.
- Agent posts run updates to Slack:
  - plan start,
  - current stage/pipeline progress,
  - manual intervention required,
  - failures and useful runtime events.
- Frequency-based updates are configurable.
- Critical events (manual gate/failure) override frequency and send immediately.
- Runtime controls during execution:
  - change frequency while running,
  - silence/unsilence notifications,
  - remove/stop agent while plan continues,
  - add/enable agent at runtime.

### 3.10 Logs and Observability
- User can view logs on demand from dashboard when expanding steps/jobs.
- User can open logs in expanded window/modal for comfortable reading.

### 3.11 Failure Recovery
- On plan failure, user can resume from failed step.
- If user does not want resume, user can start over from beginning.
- Resume should capture failure position clearly (example: before-stage Slack, stage pipelines, after-stage Slack).

## 4. Persistence and Sharing Requirements
- Persist all groups and choices across sessions.
- Shareable across machines/users via JSON import/export:
  - plan state (tabs, stage mapping, inter-stage config, pipeline config),
  - variable presets,
  - repo set.
- Preserve backward compatibility for stored/exported schema where practical.

## 5. UI/UX Requirements
- Dashboard should be cleaner and intuitive.
- Inter-stage controls should be easy to discover and edit.
- Slack/manual steps should not look cluttered.
- Clear distinction between:
  - Run Plan,
  - Resume from Failed Step,
  - Start Over.
- Support desktop and mobile responsive behavior.

## 6. Reliability / Bug Fix Expectations
- Branch filtering must include expected branches (example: `main`).
- Persisted repos must not disappear unexpectedly.
- Workflows should reappear after startup using restored repo state.
- Configuration edits must survive re-render/navigation.

## 7. Suggested Data Model (Implementation Guidance)
- Custom tab object should include:
  - `id`, `name`, `workflowKeys`,
  - `stageByKey`,
  - `pipelineConfigsByKey` (branch + `inputsByName`),
  - `manualApprovalAfterStage`,
  - `slackNotificationByStage`,
  - `agentConfig`.
- Runtime object per tab should include:
  - `running`, `stageLabel`, `lastMessage`, `results`,
  - manual gate state,
  - agent runtime state,
  - failure resume cursor (`resumeStep`).

## 8. Integration Assumptions
- GitHub workflows are dispatchable only if `workflow_dispatch` exists.
- GitHub token must have appropriate permissions (`actions:read`, `actions:write` where needed).
- Slack delivery should handle partial webhook failures and report errors clearly.

## 9. Acceptance Checklist
- Can create named custom tabs from selected workflows.
- Can add/remove/move workflows across tabs.
- Can set stage numbers and run sequential/parallel plan correctly.
- Can configure and persist per-pipeline workflow inputs.
- Can add/remove manual and Slack steps between stages and at boundaries.
- Slack supports multi-webhook and template CRUD.
- Agent sends periodic + critical immediate updates and is controllable at runtime.
- Repo/workflow state persists across app restarts and can be imported/exported.
- Logs are available on demand and in expanded view.
- On failure: both resume and start-over paths are available and functional.

