# GitWorkflowDashboard Plan

## Objective
Deliver a reliable workflow orchestration dashboard for multi-repo deployments with strong runtime control, visibility, and portability.

## Delivered
1. Custom named tabs built from selected pipelines.
2. Stage-based execution with parallel pipelines per stage.
3. Manual approval steps between stages.
4. Slack steps before/after stage boundaries, including before stage 1 and after final stage.
5. Slack templates with create/update/delete and selection per boundary.
6. Multi-webhook Slack delivery for shared notifications.
7. Agent/bot runtime controls (enable, silence, remove, frequency update).
8. Per-pipeline variables sidebar (branch + workflow inputs).
9. Import/export for plan state, variable presets, and repos.
10. On-demand logs and expanded log viewer.
11. Failure handling with resume cursor and explicit start-over option.

## Near-Term Stabilization
1. Add regression tests for resume cursor transitions:
   - `before_slack` resume
   - `stage` resume (failed pipelines only)
   - `after_slack` resume
2. Add tests for manual-gate + resume interactions.
3. Add compatibility checks for import/export schema versions.
4. Improve traceability from plan stage to run IDs in history.

## Operating Rules
1. Keep docs and behavior aligned after each feature change.
2. Preserve existing localStorage keys unless a migration path is provided.
3. Validate every change with:
   - `cargo test`
   - inline script parse check for `src/frontend/index.html`

## Linked Detailed Plan
- `memory/plan-GitWorkflowDashboard.md`
