# GitWorkflowDashboard Plan

## Objective
Ship a reliable multi-repo workflow orchestration dashboard where users can build reusable run plans, configure per-pipeline inputs, and execute with clear control + observability.

## Delivered
- Custom named workflow tabs created from selected pipelines.
- Stage-based execution with parallel pipelines per stage.
- Manual intervention gates between stages.
- Slack step insertion/removal before/after stages, including before stage 1 and after last stage.
- Slack templates (CRUD) and multi-webhook support.
- Plan runtime agent with Slack status updates, runtime frequency control, silence/remove controls, and forced critical notifications.
- Per-pipeline variables sidebar with workflow_dispatch input support and persisted values.
- Pipeline movement/removal across tabs and custom tab reordering.
- State export/import for plan state, variable presets, and repos.
- On-demand logs and expanded log view.
- Resume from failed step + start over controls.

## Stabilization Priorities
1. Add focused regression tests around plan runtime state transitions:
   - stage failure -> resume cursor
   - resume from `before_slack` / `stage` / `after_slack`
   - manual gate + resume interplay
2. Add UI smoke test coverage for import/export schema compatibility.
3. Add guardrails for stale resume cursor after tab/stage edits during downtime.
4. Improve run history correlation from plan runtime to pipeline run IDs for easier auditability.
5. Add explicit versioning notes for exported JSON schema upgrades.

## Operational Checklist
- Keep `README.md` and `memory/*` docs in sync whenever orchestration behavior changes.
- Run validation on each change:
  - `cargo test`
  - inline frontend script parse check
- Preserve backward compatibility for localStorage and import/export JSON where possible.
