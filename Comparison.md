# GitWorkflowDashboard vs. Alternatives

A comparison of GitWorkflowDashboard against existing GitHub Actions monitoring tools.

## Open Source / Self-Hosted

### [github-action-dashboard](https://github.com/chriskinsman/github-action-dashboard)
Node.js app using GitHub App authentication with webhook support. Monitors workflow status across repos but lacks interactivity beyond viewing status.

### [GitactionBoard](https://github.com/otto-de/gitactionboard)
Java-based radiator-style status board. Focuses on pass/fail visibility for team displays. Minimal interactivity.

### [lf-workflow-dash](https://github.com/lincc-frameworks/lf-workflow-dash)
Python-based static HTML generator deployed via GitHub Pages. Refreshes every 15 minutes through a scheduled workflow. No real-time updates or interactivity.

## SaaS / Commercial

### [Octolense](https://github.com/marketplace/octolense-github-actions-dashboard)
GitHub Marketplace app with real-time webhook monitoring and cost insights. Commercial product.

### [CICube](https://cicube.io/blog/github-actions-dashboard/)
CI analytics platform tracking success rate, MTTR, duration, and throughput. Commercial product.

### [Datadog CI Visibility](https://www.datadoghq.com/blog/datadog-github-actions-ci-visibility/)
Enterprise-grade observability using OpenTelemetry tracing for GitHub Actions pipelines. Requires Datadog subscription.

### [Dash0](https://www.dash0.com/guides/github-actions-observability-opentelemetry-tracing)
OpenTelemetry-based observability platform with GitHub Actions tracing and performance analytics.

## Feature Comparison

| Feature | GitWorkflowDashboard | github-action-dashboard | GitactionBoard | lf-workflow-dash |
|---|---|---|---|---|
| Real-time WebSocket updates | Yes | No | No | No |
| Inline job logs (ANSI stripped) | Yes | No | No | No |
| Workflow dispatch / re-run | Yes | No | No | No |
| Cancel running workflows | Yes | No | No | No |
| Webhook + polling hybrid | Yes | Webhook only | Polling | Scheduled |
| Single binary deployment | Yes (Rust) | No (Node.js) | No (Java) | No (Python) |
| Embedded frontend (no build step) | Yes | No (React) | No | No |
| ETag caching / rate limit aware | Yes | No | No | No |
| Dynamic repo management (add/remove at runtime) | Yes | Config only | Config only | YAML config |
| Docker support | Yes | Yes | Yes | No |
| Authentication / CSRF protection | Yes | GitHub App | No | N/A (static) |
| Multi-repo sidebar filtering | Yes | No | No | No |

## What Sets GitWorkflowDashboard Apart

- **Single binary, zero dependencies** -- Rust backend with an embedded HTML frontend served via `include_str!`. No Node.js, no npm, no build toolchain for the frontend.
- **Real-time updates** -- WebSocket broadcast channel pushes state changes instantly. No polling from the browser.
- **Inline job logs** -- View GitHub Actions job logs directly in the dashboard with ANSI code stripping and step-level parsing. No need to leave the dashboard.
- **Full workflow control** -- Dispatch workflows, re-run failed runs, and cancel in-progress runs without opening GitHub.
- **Rate limit intelligence** -- ETag caching minimizes API quota usage. Polling automatically pauses when rate limits are low.
- **Lightweight** -- Single-file ~5000-line frontend, ~2500-line Rust backend. Runs comfortably on minimal hardware.
