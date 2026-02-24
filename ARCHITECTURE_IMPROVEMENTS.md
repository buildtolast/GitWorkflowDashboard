# Architecture Improvements — v2.7

**Date:** 2026-02-24
**Branch:** `feature/architecture-improvements`
**Commit:** `35c794e`

---

## Overview

v2.7 implements three architectural improvements based on an external analysis of the GitWorkflowDashboard codebase. The changes target API resilience, data durability, and server-side computation — addressing the highest-value gaps without disrupting the existing architecture.

**Result:** 273 tests pass (62 lib + 106 binary + 105 integration), including 27 new tests covering all three improvements.

---

## Phase 1: Exponential Backoff with Jitter

### Problem

The background poller used a fixed interval (default 30s) regardless of GitHub API rate limit pressure. Under heavy use or with multiple instances, this could exhaust rate limits and trigger 403 responses.

### Solution

Adaptive three-tier rate limit response in the poller:

| Rate Limit Remaining | Behavior |
|---|---|
| **> 100** | Normal polling at base interval |
| **10–100** | Graduated backoff: `base x 2^n + jitter` |
| **< 10** | Skip poll cycle entirely |

**Formula:** `min(base x 2^n + random_jitter(0..base), 300 seconds)`

- `n` = consecutive low-rate-limit cycles (capped at 5 to prevent overflow)
- Jitter range `[0, base)` prevents thundering herd on multi-instance deployments
- Maximum backoff capped at 5 minutes
- Auto-recovery: `n` resets to 0 when rate limit recovers above threshold

### Key Constants

```rust
const BACKOFF_THRESHOLD: u32 = 100;   // Backoff activates below this
const CRITICAL_THRESHOLD: u32 = 10;    // Polls skipped below this
const MAX_BACKOFF_POWER: u32 = 5;      // 2^5 = 32x multiplier cap
const MAX_BACKOFF_SECS: u64 = 300;     // 5-minute ceiling
```

### Implementation

**File:** `src/github/poller.rs`

- Changed from fixed `tokio::time::interval` to `tokio::time::sleep(dynamic_duration)`
- New `compute_backoff_interval(base, consecutive_low)` function
- Poller checks rate limit before each cycle and adjusts behavior
- Structured logging at each tier transition (warn for critical, info for backoff/recovery)

**Tests:** 8 unit tests covering zero backoff, graduated powers, jitter bounds, max cap, and overflow safety.

---

## Phase 2: SQLite Persistence Layer

### Problem

Repository list was stored in flat JSON (`~/.gwd/repos.json`). Workflow run data existed only in memory — lost on every server restart. No historical data survived across sessions, limiting analytics to what was fetched in the current session.

### Solution

SQLite database (`~/.gwd/dashboard.db`) via `rusqlite` with the `bundled` feature (statically compiled SQLite, zero runtime dependencies).

### Schema

```sql
CREATE TABLE repos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner TEXT NOT NULL,
    name TEXT NOT NULL,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(owner, name)
);

CREATE TABLE workflow_runs (
    id INTEGER PRIMARY KEY,            -- GitHub run ID
    repo_id INTEGER NOT NULL,
    workflow_name TEXT,
    workflow_id INTEGER,
    status TEXT,                        -- queued, in_progress, completed
    conclusion TEXT,                    -- success, failure, cancelled, etc.
    actor TEXT,
    event TEXT,                         -- push, pull_request, workflow_dispatch, schedule
    branch TEXT,
    head_sha TEXT,
    run_number INTEGER,
    started_at TEXT,
    completed_at TEXT,
    duration_secs INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    html_url TEXT,
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE analytics_cache (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    computed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Performance indexes
CREATE INDEX idx_runs_repo ON workflow_runs(repo_id);
CREATE INDEX idx_runs_conclusion ON workflow_runs(conclusion);
CREATE INDEX idx_runs_started ON workflow_runs(started_at);
CREATE INDEX idx_runs_event ON workflow_runs(event);
CREATE INDEX idx_runs_actor ON workflow_runs(actor);
CREATE INDEX idx_runs_workflow ON workflow_runs(workflow_id);
```

### Configuration

- **WAL mode** — Enables concurrent reads while the poller writes
- **Busy timeout** — 5000ms to handle lock contention gracefully
- **Foreign keys** — Enforced for referential integrity (cascade deletes)
- **Idempotent migrations** — `CREATE TABLE IF NOT EXISTS` runs on every startup

### Auto-Migration from JSON

On first run, if `repos.json` exists and the repos table is empty:
1. Parse existing JSON file
2. Insert all repos into SQLite
3. Rename `repos.json` to `repos.json.bak` to prevent re-migration

### Run Persistence

The poller calls `upsert_runs()` after every poll cycle. Uses `ON CONFLICT(id) DO UPDATE` for idempotent writes — the same run can be upserted multiple times as its status progresses from `queued` to `in_progress` to `completed`.

### Thread Safety

`rusqlite::Connection` is `Send` but not `Sync`. The `Database` struct uses `unsafe impl Sync for Database {}` because access is protected through `Arc` and only accessed from single async contexts at a time.

**Type alias:** `SharedDatabase = Arc<Option<Database>>` — the `Option` handles cases where the data directory is unavailable or database creation fails (falls back to legacy JSON loading).

### Files Modified

| File | Change |
|---|---|
| `Cargo.toml` | Added `rusqlite = { version = "0.32", features = ["bundled"] }` |
| `src/database.rs` (new) | Schema, migrations, CRUD, analytics queries, cache (~960 lines) |
| `src/main.rs` | Database initialization with JSON fallback |
| `src/handlers/repos.rs` | Persist via database instead of repo_store |
| `src/github/poller.rs` | `upsert_runs()` after each poll cycle |
| `src/lib.rs` | Added `pub mod database` |

**Tests:** 14 unit tests covering repo CRUD, run upsert, analytics queries, JSON migration, and cache TTL.

---

## Phase 3: Server-Side Analytics API

### Problem

Analytics were computed entirely client-side in JavaScript, iterating over whatever runs happened to be in memory. This was CPU-intensive in the browser, inconsistent across sessions, and limited to the current session's data.

### Solution

New API endpoint that computes analytics server-side via SQL queries against the SQLite database.

### Endpoint

```
GET /api/analytics/summary?range=7d
```

**Supported ranges:** `1h`, `6h`, `12h`, `1d`, `3d`, `7d` (default), `30d`

### Response Payload

```json
{
  "time_range": "7d",
  "success_rate": {
    "total_runs": 150,
    "successful_runs": 130,
    "overall_rate": 86.7,
    "per_repo": [{"repo": "owner/name", "total": 50, "successes": 45, "rate": 90.0}],
    "per_workflow": [{"workflow": "CI", "total": 80, "successes": 75, "rate": 93.75}]
  },
  "duration": {
    "average_secs": 245,
    "median_secs": 180,
    "per_workflow": [{"workflow": "CI", "avg_secs": 200, "count": 80}],
    "slowest_runs": [{"id": 12345, "workflow": "Deploy", "duration_secs": 1200}]
  },
  "triggers": [
    {"event": "push", "count": 80, "success_rate": 90.0},
    {"event": "pull_request", "count": 50, "success_rate": 85.0}
  ],
  "actors": [
    {"actor": "octocat", "count": 40, "success_rate": 92.5}
  ],
  "failures": {
    "most_failing": [{"workflow": "Deploy", "failure_count": 10}],
    "recent_failures": [{"id": 12346, "workflow": "Deploy", "conclusion": "failure"}]
  },
  "branches": [
    {"branch": "main", "total": 60, "successes": 55, "failures": 5}
  ]
}
```

### Caching

Results are cached in the `analytics_cache` SQLite table with a **60-second TTL**. The cache key is `analytics_summary_{range}`. Stale entries are ignored and recomputed on next request.

### Files Added/Modified

| File | Change |
|---|---|
| `src/handlers/analytics.rs` (new) | Endpoint handler, range parsing, cache logic |
| `src/handlers/mod.rs` | Registered `pub mod analytics` |
| `src/main.rs` | Added `/api/analytics/summary` route |
| `src/database.rs` | Analytics query methods + response types |

**Tests:** 5 unit tests for range parsing and defaults.

---

## What Was NOT Implemented (and Why)

These recommendations from the external analysis were evaluated and intentionally deferred:

| Recommendation | Reason for Deferral |
|---|---|
| **GraphQL API migration** | Massive refactoring effort; ETag caching already minimizes redundant API calls |
| **Octocrab client library** | `reqwest` + custom `GitHubClient` is well-tested and fits current needs; octocrab adds pagination convenience but not worth the migration cost |
| **Actix Actor model for WebSocket** | Over-engineering for current scale; `tokio::select!` loop is simpler and adequate |
| **Cargo workspace layout** | Single crate is fine for this project size; module organization is already clean |
| **Virtualized log rendering** | 2MB server-side truncation already handles this; logs rarely exceed the limit |
| **Rust WASM frontend (Leptos/Yew)** | Already migrated to Slint native desktop UI; web UI is legacy behind `web` feature gate |
| **Web Components + bundler** | Same — Slint is the future frontend, not web |

---

## Dependencies Added

| Crate | Version | Purpose |
|---|---|---|
| `rand` | 0.8 | Jitter generation for backoff formula |
| `rusqlite` | 0.32 (bundled) | SQLite database with statically compiled library |

---

## Test Coverage

| Category | Count | What's Tested |
|---|---|---|
| Backoff | 8 | Zero/graduated/max/overflow/jitter bounds |
| Database | 14 | Repo CRUD, run upsert, analytics SQL, JSON migration, cache TTL |
| Analytics handler | 5 | Range parsing (hours, days, invalid, case-insensitive), defaults |
| **Total new** | **27** | |
| **Total project** | **273** | 62 lib + 106 binary + 105 integration |
