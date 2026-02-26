# Architecture Scaling — Bottleneck Analysis & Implementation Reference

> **Purpose:** Self-contained implementation guide for scaling GitWorkflowDashboard from ~50 to 1000+ concurrent users.
> Each section includes exact file paths, line numbers, current code, proposed changes, and verification steps so work can resume without re-analysing the codebase.
>
> **Created:** 2026-02-26 · **Branch:** `feature/merge-added-functionality-to-main`

---

## Table of Contents

1. [Current Architecture Snapshot](#1-current-architecture-snapshot)
2. [Bottleneck Inventory](#2-bottleneck-inventory)
3. [Tier 1 — Quick Wins (Hours, → ~75 Users)](#3-tier-1--quick-wins)
4. [Tier 2 — Architecture Changes (Days, → ~200 Users)](#4-tier-2--architecture-changes)
5. [Tier 3 — Full Scale (Weeks, → 500+ Users)](#5-tier-3--full-scale)
6. [Tier 4 — Enterprise (Reference Only, → 1000+ Users)](#6-tier-4--enterprise)
7. [Key Reference Data](#7-key-reference-data)
8. [Verification Checklist](#8-verification-checklist)

---

## 1. Current Architecture Snapshot

### System Diagram (text)

```
┌─────────────────────────────────────────────────────────────────┐
│  CLIENTS (Browser)                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │  WebSocket    │  │  REST /api/* │  │  Vanilla JS  │         │
│  │  (ws/live)    │  │  (fetch)     │  │  Frontend    │         │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘         │
└─────────┼──────────────────┼──────────────────┼─────────────────┘
          │                  │                  │
          ▼                  ▼                  ▼
┌─────────────────────────────────────────────────────────────────┐
│  SERVER (actix-web 4)                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │  WS Handler  │  │  API Routes  │  │  Static HTML │         │
│  │  (ws.rs)     │  │  (handlers/) │  │  (embedded)  │         │
│  └──────┬───────┘  └──────┬───────┘  └──────────────┘         │
│         │                  │                                    │
│         ▼                  ▼                                    │
│  ┌──────────────┐  ┌──────────────┐                            │
│  │  Broadcast    │  │  Shared      │                            │
│  │  Channel      │  │  RwLock      │                            │
│  │  (4096 cap)  │  │  State       │                            │
│  └──────┬───────┘  └──────┬───────┘                            │
└─────────┼──────────────────┼────────────────────────────────────┘
          │                  │
          ▼                  ▼
┌─────────────────────────────────────────────────────────────────┐
│  SERVICES                                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │  Poller      │  │  SQLite DB   │  │  Webhook     │         │
│  │  (poller.rs) │  │  (WAL mode)  │  │  (webhook.rs)│         │
│  └──────┬───────┘  └──────────────┘  └──────────────┘         │
└─────────┼───────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│  EXTERNAL                                                       │
│  ┌──────────────────────────────────────────────────┐          │
│  │  GitHub API  (5,000 req/hr rate limit)           │          │
│  └──────────────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

### Key Type Definitions

**`src/models.rs`**

```rust
// Line 138-143 — Full dashboard state snapshot
#[derive(Debug, Clone, Serialize, Default)]
pub struct DashboardState {
    pub repos: Vec<RepoState>,
    pub rate_limit: RateLimitInfo,
    pub last_poll: Option<DateTime<Utc>>,
}

// Line 145-152 — State for a single repository
#[derive(Debug, Clone, Serialize)]
pub struct RepoState {
    pub repo: String,
    pub workflows: Vec<Workflow>,
    pub active_runs: Vec<WorkflowRun>,
    pub recent_runs: Vec<WorkflowRun>,
}

// Line 168-186 — WebSocket message types
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "state")]
    StateUpdate { data: DashboardState },
    #[serde(rename = "run_update")]
    RunUpdate { repo: String, run: WorkflowRun },
    #[serde(rename = "rate_limit")]
    RateLimitUpdate { data: RateLimitInfo },
    #[serde(rename = "repos_changed")]
    ReposChanged { repos: Vec<String> },
}

// Line 374-375 — Shared state type aliases
pub type SharedDashboardState = Arc<RwLock<DashboardState>>;
pub type SharedRepoList = Arc<RwLock<Vec<RepoRef>>>;
```

### Critical Constants

| Constant | File | Line | Value |
|----------|------|------|-------|
| Broadcast channel capacity | `src/main.rs` | 136 | `4096` |
| Worker threads | `src/main.rs` | 181 | default (num_cpus, no `.workers()` call) |
| WS connection limit | `src/handlers/ws.rs` | 12 | `50` |
| ETag cache max size | `src/github/client.rs` | 748 | `1000` |
| Max runs per repo (prune) | `src/database.rs` | 23 | `5000` |
| Analytics cache TTL | `src/handlers/analytics.rs` | 65 | `60` seconds |
| Rate limit backoff threshold | `src/github/poller.rs` | 27 | `100` remaining |
| Rate limit critical threshold | `src/github/poller.rs` | 30 | `10` remaining |
| Max backoff duration | `src/github/poller.rs` | 36 | `300` seconds (5 min) |
| Rate limiter (API) | `src/main.rs` | 176-177 | `1 req/sec sustain`, `30 burst` |

### Current Dependencies (`Cargo.toml`)

```toml
actix-web = "4"
actix-ws = "0.3"
actix-rt = "2"
actix-cors = "0.7"
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["full"] }
tokio-util = "0.7"
futures-util = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
base64 = "0.22"
urlencoding = "2"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-actix-web = "0.7"
actix-governor = "0.6"
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
log = "0.4"
dirs = "6"
rand = "0.8"
rusqlite = { version = "0.32", features = ["bundled"] }
```

**NOT present** (needed for scaling): `futures` (full crate), `lru`, `dashmap`, `arc-swap`

---

## 2. Bottleneck Inventory

### Critical (C) — Blocks beyond ~50 users

| ID | Description | File | Lines | Impact |
|----|-------------|------|-------|--------|
| C1 | Sequential repo polling — 50 repos × ~120ms = 6+ sec per cycle | `poller.rs` | 179-218 | Stale data, wasted rate limit budget waiting on I/O |
| C2 | Full `DashboardState.clone()` on every broadcast — multi-MB JSON per poll cycle | `poller.rs` | 251-252 | O(N×clients) memory allocation every 30s |
| C3 | `renderAll()` rebuilds all 8+ tabs on every WS message — 400+ DOM ops/min | `js/04-render.js` | 5-19 | UI janks, dropped frames, high GC pressure |
| C4 | WS connection limit hardcoded to 50 with race condition | `ws.rs` | 12, 57-76 | Hard ceiling at 50 concurrent dashboard viewers |

### High (H) — Degrades experience at 30-75 users

| ID | Description | File | Lines | Impact |
|----|-------------|------|-------|--------|
| H1 | Single SQLite connection (no pool) — all writes serialize | `database.rs` | 26-29 | Write contention during concurrent polls + analytics |
| H2 | Individual INSERT per run (no batching) — N round-trips per poll | `database.rs` | 266-314 | 1000+ individual SQLite executions per cycle |
| H3 | Analytics cache stampede — 60s TTL, no coalescing | `analytics.rs` | 63-86 | 50 clients simultaneously recompute 6 SQL queries |
| H4 | ETag cache random eviction (not LRU) | `client.rs` | 748-763 | Evicts hot entries, wastes conditional request savings |
| H5 | Queued runs fetched with separate `.await` (not in `join!()`) | `poller.rs` | 293-299 | +120ms latency per repo per poll cycle |
| H6 | `previous_runs.write()` lock held for entire diff loop | `poller.rs` | 183-207 | Write lock contention blocks concurrent reads |
| H7 | Search filters have no debounce — full table rebuild per keystroke | `js/06-app.js` | 754-769 | CPU spikes during typing in search boxes |
| H8 | Duration timers run even for hidden tabs | `js/04-render.js` | 911-937 | Wasted CPU cycles + DOM queries for invisible content |

### Medium (M) — Performance tax at any scale

| ID | Description | File | Lines | Impact |
|----|-------------|------|-------|--------|
| M1 | No compound database indexes (6 single-column only) | `database.rs` | 92-97 | Full table scans on multi-column analytics queries |
| M2 | `PreviousRunTracker` unbounded — only inserts, never evicts | `poller.rs` | 15-20 | Unbounded memory growth over days of operation |
| M3 | No pre-flight rate limit budget check before poll batch | `poller.rs` | 80-114 | May start poll batch with insufficient budget for all repos |
| M4 | Rate limit updates reactive (after request) not proactive | `client.rs` | 771+ | Can overshoot by one request before noticing limit exhaustion |
| M5 | No `Content-Length` check before downloading large log files | `client.rs` | log fns | 100MB+ logs can exhaust memory |
| M6 | Poller has no watchdog — panic = silent death | `main.rs` | 157-169 | Single `tokio::spawn` with no restart-on-panic |
| M7 | No circuit breaker for failing repos | `poller.rs` | 210-217 | Erroring repos consume rate limit every cycle |
| M8 | `handleReposChanged()` calls 7 individual render functions | `js/03-websocket.js` | 321-334 | 7 DOM rebuilds instead of one coordinated update |

### Fault Tolerance Gaps (F)

| ID | Description | File | Lines | Impact |
|----|-------------|------|-------|--------|
| F1 | WS connection counter uses `Ordering::Relaxed` — race condition | `ws.rs` | 57, 76 | Can exceed MAX_WS_CONNECTIONS under concurrent connects |
| F2 | No graceful degradation when SQLite is unavailable | `database.rs` | 34-50 | Server fails to start if DB open errors |
| F3 | Broadcast channel lagged clients only log warning | `ws.rs` | 133-135 | Lagged client gets no resync, just misses messages |
| F4 | No health check endpoint | `main.rs` | — | Load balancers can't verify server health |
| F5 | Single poller task — no supervision or restart | `main.rs` | 157-169 | Poller panic = no more data updates |

---

## 3. Tier 1 — Quick Wins

**Estimated effort:** Hours per item · **Target capacity:** ~75 concurrent users

### 3.1 Parallel Repo Polling (C1, H5, H6)

**File:** `src/github/poller.rs` · **Lines:** 179-218, 276-299

**Current code (sequential loop, line 179):**
```rust
for repo_ref in &repo_list {
    match poll_single_repo(client, repo_ref).await {
        Ok(repo_state) => {
            // Diff runs against previous state
            let mut prev = previous_runs.write().await;
            // ... diffing loop holding write lock ...
            repo_states.push(repo_state);
        }
        Err(e) => { /* log error */ }
    }
}
```

**Proposed change:**
```rust
use futures::stream::{self, StreamExt};

// Poll all repos concurrently (max 10 in-flight)
let results: Vec<_> = stream::iter(&repo_list)
    .map(|repo_ref| {
        let client = client.clone();
        async move {
            let result = poll_single_repo(&client, repo_ref).await;
            (repo_ref.full_name(), result)
        }
    })
    .buffer_unordered(10)
    .collect()
    .await;

// Single lock acquisition for diffing AFTER all polls complete
let mut prev = previous_runs.write().await;
for (repo_name, result) in results {
    match result {
        Ok(repo_state) => {
            // diff logic here (same as current, but lock held only once)
            for run in &repo_state.active_runs { /* ... */ }
            for run in &repo_state.recent_runs { /* ... */ }
            repo_states.push(repo_state);
        }
        Err(e) => { tracing::error!(repo = %repo_name, error = %e, "Failed to poll repo"); }
    }
}
drop(prev); // Explicit drop to release lock early
```

**Also fix `poll_single_repo` queued runs (line 276-299):**

Current: 3-way `tokio::join!()` + separate `.await` for queued runs.

```rust
// CURRENT — separate await (line 293-299)
let queued_runs = client
    .list_runs(owner, repo, Some("queued"), 20)
    .await
    .unwrap_or_else(|e| { /* ... */ vec![] });
```

**Proposed:** Add 4th call to existing `tokio::join!()`:
```rust
let (workflows_result, active_runs_result, recent_runs_result, queued_runs_result) = tokio::join!(
    client.list_workflows(owner, repo),
    client.list_runs(owner, repo, Some("in_progress"), 20),
    client.list_runs(owner, repo, None, 20),
    client.list_runs(owner, repo, Some("queued"), 20),  // NEW
);
let queued_runs = queued_runs_result.unwrap_or_else(|e| {
    tracing::warn!(repo = %repo_ref, error = %e, "Failed to fetch queued runs");
    vec![]
});
```

**New dependency:** `futures = "0.3"` in `Cargo.toml` (note: `futures-util = "0.3"` is already present, but `futures::stream` needs the full crate, or use `futures-util::stream` directly which should also work).

---

### 3.2 Render Only Visible Tab (C3)

**File:** `src/frontend/js/04-render.js` · **Lines:** 5-19

**Current code:**
```javascript
function renderAll() {
  renderRateLimit();
  renderLastPoll();
  renderSidebar();
  renderActiveRuns();
  renderWorkflows();
  renderActions();
  renderHistory();
  renderWhatsHappening();
  renderAnalytics();
  if (State.activeTab && State.activeTab.indexOf('custom-workflow-') === 0) {
    App.renderCustomWorkflowTab();
  }
}
```

**Proposed change:**
```javascript
// Track which tabs have stale data needing re-render
var _tabDirty = {
  'active-runs': true, 'workflows': true, 'actions': true,
  'history': true, 'whats-happening': true, 'analytics': true
};

function renderAll() {
  // Always render: header elements (tiny cost)
  renderRateLimit();
  renderLastPoll();
  renderSidebar();

  // Mark all tabs dirty
  Object.keys(_tabDirty).forEach(function(k) { _tabDirty[k] = true; });

  // Only render the currently visible tab
  _renderTab(State.activeTab);
}

function _renderTab(tab) {
  switch (tab) {
    case 'active-runs':   renderActiveRuns(); startDurationTimers(); break;
    case 'workflows':     renderWorkflows(); break;
    case 'actions':       renderActions(); break;
    case 'history':       renderHistory(); break;
    case 'whats-happening': renderWhatsHappening(); break;
    case 'analytics':     renderAnalytics(); break;
    default:
      if (tab && tab.indexOf('custom-workflow-') === 0) {
        App.renderCustomWorkflowTab();
      }
  }
  _tabDirty[tab] = false;
}
```

**Also update `App.switchTab()` in `src/frontend/js/06-app.js`:**
```javascript
// Add to switchTab() after setting State.activeTab:
if (_tabDirty[State.activeTab]) {
  _renderTab(State.activeTab);
}
```

---

### 3.3 Search Filter Debounce (H7)

**File:** `src/frontend/js/02-utils.js` (add utility) + `src/frontend/js/06-app.js` lines 754-769

**Add to `js/02-utils.js`:**
```javascript
function debounce(fn, delay) {
  var timer;
  return function() {
    var ctx = this, args = arguments;
    clearTimeout(timer);
    timer = setTimeout(function() { fn.apply(ctx, args); }, delay);
  };
}
```

**Update `js/06-app.js` filter functions:**

Current (line 754-769):
```javascript
filterWorkflows: function() {
  State.workflowSearchQuery = document.getElementById('workflow-search-input').value;
  renderWorkflows();
},
filterActions: function() {
  State.actionsSearchQuery = document.getElementById('actions-search-input').value;
  renderActions();
},
filterHistory: function() {
  State.historySearchQuery = document.getElementById('history-search-input').value;
  renderHistory();
},
```

**Proposed:**
```javascript
filterWorkflows: debounce(function() {
  State.workflowSearchQuery = document.getElementById('workflow-search-input').value;
  renderWorkflows();
}, 150),
filterActions: debounce(function() {
  State.actionsSearchQuery = document.getElementById('actions-search-input').value;
  renderActions();
}, 150),
filterHistory: debounce(function() {
  State.historySearchQuery = document.getElementById('history-search-input').value;
  renderHistory();
}, 150),
```

---

### 3.4 Compound Database Indexes (M1)

**File:** `src/database.rs` · **Lines:** 92-97 (after existing 6 indexes)

**Current indexes:**
```sql
CREATE INDEX IF NOT EXISTS idx_runs_repo ON workflow_runs(repo_id);
CREATE INDEX IF NOT EXISTS idx_runs_conclusion ON workflow_runs(conclusion);
CREATE INDEX IF NOT EXISTS idx_runs_started ON workflow_runs(started_at);
CREATE INDEX IF NOT EXISTS idx_runs_event ON workflow_runs(event);
CREATE INDEX IF NOT EXISTS idx_runs_actor ON workflow_runs(actor);
CREATE INDEX IF NOT EXISTS idx_runs_workflow ON workflow_runs(workflow_id);
```

**Add these compound indexes (append after line 97):**
```sql
-- Analytics: success rate per repo (filters on repo_id + status)
CREATE INDEX IF NOT EXISTS idx_runs_repo_status ON workflow_runs(repo_id, status);

-- Analytics: failure analysis per repo with time range
CREATE INDEX IF NOT EXISTS idx_runs_repo_conclusion_started ON workflow_runs(repo_id, conclusion, started_at);

-- Analytics: workflow trends over time
CREATE INDEX IF NOT EXISTS idx_runs_workflow_created ON workflow_runs(workflow_id, created_at);

-- Analytics: branch activity with status
CREATE INDEX IF NOT EXISTS idx_runs_branch_conclusion ON workflow_runs(branch, conclusion);
```

---

### 3.5 Stop Hidden Tab Timers (H8)

**File:** `src/frontend/js/04-render.js` · **Lines:** 911-937

**Current:** `startDurationTimers()` is called from `renderAll()` → `renderActiveRuns()` regardless of which tab is visible.

**Proposed:** Only start timers when active-runs tab is visible. Handled by Tier 1 §3.2 change — `_renderTab()` only calls `startDurationTimers()` for `'active-runs'` tab. Additionally, stop timers on tab switch away:

```javascript
// Add to App.switchTab() when leaving active-runs tab:
if (previousTab === 'active-runs') {
  stopAllDurationTimers();
}
```

---

### 3.6 Raise WS Connection Limit (C4 partial)

**File:** `src/handlers/ws.rs` · **Line:** 12

**Current:**
```rust
const MAX_WS_CONNECTIONS: usize = 50;
```

**Proposed:**
```rust
const MAX_WS_CONNECTIONS: usize = 500;
```

> Note: The race condition (F1) should also be fixed — see Tier 2 §4.8.

---

## 4. Tier 2 — Architecture Changes

**Estimated effort:** Days per item · **Target capacity:** ~200 concurrent users

### 4.1 Delta WebSocket Messages (C2)

**Files:** `src/github/poller.rs` (247-264), `src/models.rs` (168-186), `src/handlers/ws.rs` (110-119), `src/frontend/js/03-websocket.js`

**Goal:** Only broadcast changed data instead of full state clone every cycle.

**Step 1 — Add `StateDelta` variant to `WsMessage` (models.rs):**
```rust
/// Incremental changes since last broadcast.
#[serde(rename = "state_delta")]
StateDelta {
    changed_repos: Vec<RepoState>,     // repos with changed runs
    removed_repos: Vec<String>,         // repos no longer tracked
    rate_limit: RateLimitInfo,
    last_poll: Option<DateTime<Utc>>,
},
```

**Step 2 — Compute diff in poller (poller.rs, after line 245):**
```rust
// Compare new repo_states against previous broadcast
// Only include repos where active_runs or recent_runs changed
let changed_repos: Vec<RepoState> = repo_states.iter()
    .filter(|rs| {
        // Compare against last broadcast snapshot
        // (store previous in Arc<RwLock<DashboardState>>)
        true // simplified — real impl compares run IDs/statuses
    })
    .cloned()
    .collect();

// Send delta to existing clients, full state only for initial connect
if !changed_repos.is_empty() {
    let _ = ws_tx.send(WsMessage::StateDelta {
        changed_repos,
        removed_repos: vec![],
        rate_limit: updated_rate_limit.clone(),
        last_poll: Some(chrono::Utc::now()),
    });
}
```

**Step 3 — Keep `StateUpdate` for initial connect only (ws.rs line 110-119):**
No change needed — initial state is already sent via `StateUpdate` on connect.

**Step 4 — Frontend handler (js/03-websocket.js):**
```javascript
case 'state_delta':
  handleStateDelta(msg);
  break;

function handleStateDelta(msg) {
  // Merge changed repos into State.repoStates
  msg.changed_repos.forEach(function(updatedRepo) {
    var idx = State.repoStates.findIndex(function(r) { return r.repo === updatedRepo.repo; });
    if (idx >= 0) {
      State.repoStates[idx] = updatedRepo;
    } else {
      State.repoStates.push(updatedRepo);
    }
  });
  // Remove repos no longer tracked
  msg.removed_repos.forEach(function(repo) {
    State.repoStates = State.repoStates.filter(function(r) { return r.repo !== repo; });
  });
  State.rateLimit = msg.rate_limit || State.rateLimit;
  State.lastPoll = msg.last_poll;
  renderAll(); // will be optimized by §3.2 to only render visible tab
}
```

---

### 4.2 SQLite Batch Inserts (H2)

**File:** `src/database.rs` · **Lines:** 249-315

**Current:** Individual `self.conn.execute()` per run inside a `for run in runs` loop.

**Proposed:** Wrap in a single transaction (pattern already exists at line 190 `save_repos()`):
```rust
pub fn upsert_runs(&self, repo_owner: &str, repo_name: &str, runs: &[crate::models::WorkflowRun]) {
    let repo_id = match self.ensure_repo_id(repo_owner, repo_name) {
        Some(id) => id,
        None => return,
    };

    // Batch all upserts in a single transaction
    let tx = match self.conn.unchecked_transaction() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(error = %e, "Failed to begin transaction for run upserts");
            return;
        }
    };

    for run in runs {
        // ... same INSERT logic as current (lines 267-313) ...
        let _ = tx.execute(/* same SQL */, params![/* same params */]);
    }

    if let Err(e) = tx.commit() {
        tracing::error!(error = %e, "Failed to commit run upserts");
    }
}
```

**Impact:** Reduces N SQLite round-trips to 1 (BEGIN + N INSERTs + COMMIT).

---

### 4.3 Circuit Breaker per Repo (M7)

**File:** `src/github/poller.rs` (new struct + modify `poll_all_repos`)

**New struct:**
```rust
#[derive(Debug, Clone)]
struct RepoHealth {
    consecutive_failures: u32,
    last_failure: Option<std::time::Instant>,
    circuit_open: bool,
}

const CIRCUIT_OPEN_THRESHOLD: u32 = 3;      // open after 3 consecutive failures
const CIRCUIT_COOLDOWN: Duration = Duration::from_secs(300); // 5 min cooldown
```

**Integrate into `poll_all_repos()` (before calling `poll_single_repo`):**
```rust
// Tracker type: Arc<RwLock<HashMap<String, RepoHealth>>>
let health = repo_health.read().await;
let repo_name = repo_ref.full_name();
if let Some(h) = health.get(&repo_name) {
    if h.circuit_open {
        if h.last_failure.map(|t| t.elapsed() < CIRCUIT_COOLDOWN).unwrap_or(false) {
            tracing::debug!(repo = %repo_name, "Circuit open — skipping poll");
            continue; // skip this repo
        }
        // Cooldown elapsed — try half-open
    }
}
// ... poll repo ...
// On success: reset health
// On error: increment consecutive_failures, open circuit if threshold reached
```

---

### 4.4 Analytics Cache Coalescing (H3)

**File:** `src/handlers/analytics.rs` · **Lines:** 63-86

**Goal:** Prevent cache stampede — first request computes, concurrent requests await same result.

**Add to handler state:**
```rust
use tokio::sync::Mutex;
use std::collections::HashMap;

// In app_data setup (main.rs):
type InflightMap = Arc<Mutex<HashMap<String, tokio::sync::watch::Receiver<Option<String>>>>>;
```

**Modified handler logic:**
```rust
// Check if another request is already computing this key
let mut inflight = inflight_map.lock().await;
if let Some(rx) = inflight.get(&cache_key) {
    let mut rx = rx.clone();
    drop(inflight); // release lock while waiting
    rx.changed().await.ok();
    if let Some(cached) = &*rx.borrow() {
        return Ok(HttpResponse::Ok().content_type("application/json").body(cached.clone()));
    }
}

// First request — create watch channel
let (tx, rx) = tokio::sync::watch::channel(None);
inflight.insert(cache_key.clone(), rx);
drop(inflight);

// Compute analytics...
let json = serde_json::to_string(&summary)?;
tx.send(Some(json.clone())).ok();

// Clean up inflight entry
inflight_map.lock().await.remove(&cache_key);
```

---

### 4.5 LRU ETag Cache (H4)

**File:** `src/github/client.rs` · **Lines:** 748-763

**Current:** `HashMap` with random eviction of 10% when full.
```rust
const MAX_ETAG_CACHE_SIZE: usize = 1000;
if cache.len() >= MAX_ETAG_CACHE_SIZE {
    let to_remove: Vec<String> = cache
        .keys()
        .take(MAX_ETAG_CACHE_SIZE / 10) // random iteration order!
        .cloned()
        .collect();
    for key in to_remove { cache.remove(&key); }
}
```

**Proposed:** Replace with `lru::LruCache`:
```rust
// In GitHubClient struct:
etag_cache: Arc<RwLock<lru::LruCache<String, (String, String)>>>,

// In GitHubClient::new():
etag_cache: Arc::new(RwLock::new(lru::LruCache::new(
    std::num::NonZeroUsize::new(1000).unwrap()
))),

// In conditional request logic — just insert, LRU handles eviction:
let mut cache = self.etag_cache.write().await;
cache.put(url.to_string(), (etag, body.clone()));
```

**New dependency:** `lru = "0.12"` in `Cargo.toml`

---

### 4.6 Poller Watchdog (M6, F5)

**File:** `src/main.rs` · **Lines:** 157-169

**Current:**
```rust
tokio::spawn(async move {
    github::poller::run_poller(/* ... */).await;
});
```

**Proposed:**
```rust
tokio::spawn(async move {
    loop {
        let result = tokio::spawn(github::poller::run_poller(
            poller_client.clone(),
            poller_state.clone(),
            poller_repos.clone(),
            poller_ws_tx.clone(),
            poll_interval,
            poller_shutdown.clone(),
            poller_tracker.clone(),
            poller_db.clone(),
        )).await;

        match result {
            Ok(()) => {
                tracing::info!("Poller exited normally (shutdown)");
                break;
            }
            Err(e) => {
                tracing::error!(error = %e, "Poller panicked — restarting in 5 seconds");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
});
```

---

### 4.7 Pre-flight Rate Limit Budget Check (M3)

**File:** `src/github/poller.rs` · Before `poll_all_repos` call (line ~116)

**Logic:** Each repo needs ~4 API calls (workflows, active runs, recent runs, queued runs). Check budget before starting batch:

```rust
let rate_limit = client.get_rate_limit().await;
let repos_count = repo_list.read().await.len();
let budget_needed = repos_count as u32 * 4;

if rate_limit.remaining < budget_needed && rate_limit.remaining > CRITICAL_THRESHOLD {
    // Prioritize repos with active runs
    let prioritized: Vec<_> = repo_list.read().await.iter()
        .filter(|r| {
            state.read().await.repos.iter()
                .any(|rs| rs.repo == r.full_name() && !rs.active_runs.is_empty())
        })
        .cloned()
        .collect();
    tracing::warn!(
        budget = rate_limit.remaining,
        needed = budget_needed,
        polling = prioritized.len(),
        "Insufficient rate limit budget — polling active repos only"
    );
    // Poll only prioritized repos
}
```

---

### 4.8 WS Connection Count — Fix Race Condition (F1)

**File:** `src/handlers/ws.rs` · **Lines:** 57-76

**Current (race condition with Relaxed ordering):**
```rust
let current = connection_count.load(Ordering::Relaxed);
if current >= MAX_WS_CONNECTIONS { /* reject */ }
// ... gap where another request could also pass the check ...
connection_count.fetch_add(1, Ordering::Relaxed);
```

**Proposed (CAS loop with SeqCst):**
```rust
loop {
    let current = connection_count.load(Ordering::SeqCst);
    if current >= MAX_WS_CONNECTIONS {
        tracing::warn!(current, max = MAX_WS_CONNECTIONS, "WS connection rejected — limit reached");
        return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Too many WebSocket connections"
        })));
    }
    // Atomically increment only if value hasn't changed
    match connection_count.compare_exchange(
        current,
        current + 1,
        Ordering::SeqCst,
        Ordering::SeqCst,
    ) {
        Ok(_) => break,
        Err(_) => continue, // retry — another thread incremented
    }
}
```

Also update the decrement (line 87) to use `Ordering::SeqCst`:
```rust
counter.fetch_sub(1, Ordering::SeqCst);
```

---

## 5. Tier 3 — Full Scale

**Estimated effort:** Weeks · **Target capacity:** 500+ concurrent users

### 5.1 Full Delta Protocol with Versioning

Extend Tier 2 §4.1 with version numbers so clients can request re-sync if they miss messages:

- Add `version: u64` to `DashboardState` (increment on each poll)
- Client sends `{"type":"resync","last_version":N}` via WS text message
- Server responds with full `StateUpdate` if client version is stale
- Handle `AggregatedMessage::Text` in `ws_session()` (currently a no-op at ws.rs:154)

### 5.2 Incremental DOM Updates

Replace `innerHTML` assignments with targeted element updates:

- **Template approach:** Use `DocumentFragment` + `cloneNode()` for initial render, then `diffChildren()` for updates
- **Key locations:** `renderActiveRuns()` (render.js:150-207), `renderWorkflows()`, `renderHistory()`
- **Pattern:**
  ```javascript
  // Instead of: container.innerHTML = html;
  // Use:
  function patchList(container, newItems, keyFn, renderFn) {
    var existing = {};
    container.childNodes.forEach(function(el) { existing[el.dataset.key] = el; });
    newItems.forEach(function(item) {
      var key = keyFn(item);
      if (existing[key]) {
        updateElement(existing[key], item); // patch in-place
        delete existing[key];
      } else {
        container.appendChild(renderFn(item)); // new element
      }
    });
    Object.values(existing).forEach(function(el) { el.remove(); }); // remove stale
  }
  ```

### 5.3 Bounded State with LRU Eviction

Cap unbounded collections in both Rust and JS:

- **`PreviousRunTracker`** (poller.rs:15): Replace `HashMap` with `lru::LruCache::new(10_000)`
- **`State.expandedRuns`** (JS): Cap at 50 entries, evict oldest on overflow
- **`State.workflowInputsByKey`** (JS): Cap at 200 entries

### 5.4 Stream Large Log Responses

**File:** `src/github/client.rs` — `get_job_log()` and `get_job_logs()`

- Add server-side range support: `GET /api/{owner}/{repo}/actions/jobs/{id}/logs?offset=N&limit=M`
- Stream response body with `actix_web::HttpResponse::streaming()` instead of buffering
- Add `Content-Length` pre-check: `HEAD` request before `GET` for logs > 2MB

### 5.5 Configure actix Worker Threads

**File:** `src/main.rs` · **Line:** 181

**Current:** No `.workers()` call → defaults to `num_cpus`.

**Proposed:**
```rust
let server = HttpServer::new(move || { /* ... */ })
    .workers(num_cpus::get() * 2) // I/O-bound workload benefits from 2× CPU threads
    .bind(/* ... */)?;
```

**New dependency:** `num_cpus = "1"` in `Cargo.toml`

### 5.6 Fine-Grained Per-Repo Locks

Replace single `Arc<RwLock<DashboardState>>` with per-repo concurrent map:

```rust
// Replace SharedDashboardState with:
use dashmap::DashMap;
use arc_swap::ArcSwap;

pub struct DashboardState2 {
    pub repos: DashMap<String, RepoState>,
    pub rate_limit: ArcSwap<RateLimitInfo>,
    pub last_poll: ArcSwap<Option<DateTime<Utc>>>,
}
```

**New dependencies:**
```toml
dashmap = "6"
arc-swap = "1"
```

**Impact:** Poller can update one repo without blocking reads of other repos. WS handler reads individual repos concurrently.

---

## 6. Tier 4 — Enterprise (Reference Only)

These changes are for reference when the project requires horizontal scaling beyond a single server instance.

### 6.1 Horizontal Scaling with Redis Pub/Sub

- Replace `tokio::sync::broadcast` with Redis pub/sub
- Each server instance subscribes to a shared Redis channel
- Poller runs on a single "leader" instance (Redis-based leader election)
- WS connections distribute across instances behind a load balancer

### 6.2 PostgreSQL Replacing SQLite

- Migrate schema from SQLite to PostgreSQL
- Use `sqlx` or `diesel` with connection pooling (`deadpool-postgres`)
- Enables concurrent writes from multiple server instances
- JSONB columns for flexible metadata storage

### 6.3 GitHub App for Per-Org Rate Limits

- Register as a GitHub App instead of using personal access token
- Each installation gets its own 5,000 req/hr rate limit
- Enables per-org webhook subscriptions (no manual repo config)
- JWT-based auth with installation tokens

### 6.4 WebSocket Gateway Separation

- Dedicated WS gateway service (e.g., using `socketcluster` or custom Rust binary)
- Stateless API servers behind load balancer
- Gateway subscribes to Redis and fans out to clients
- Enables independent scaling of WS connections vs API capacity

---

## 7. Key Reference Data

### Frontend State Object (`src/frontend/js/01-state.js`)

Complete field listing (63 fields):

```javascript
const State = {
  repos: [],                          // Array of "owner/repo" strings
  repoStates: [],                     // Array of { repo, workflows, active_runs, recent_runs }
  rateLimit: { limit, remaining, used, reset },
  lastPoll: null,                     // ISO timestamp
  activeTab: 'active-runs',          // Current tab ID
  expandedRuns: {},                   // { runId: { jobs, loading } }
  expandedJobs: {},                   // { jobId: true }
  ws: null,                           // WebSocket instance
  wsReconnectTimer: null,
  wsReconnectDelay: 1000,
  wsMaxReconnectDelay: 30000,
  wsConnected: false,
  durationTimers: new Map(),          // runId -> intervalId
  sidebarCollapsed: false,
  dispatchContext: null,              // { repo, workflowId, workflowName }
  initialLoadDone: false,
  initOverlayDismissed: false,
  initEmptyTimer: null,
  workflowSearchQuery: '',
  actionsSearchQuery: '',
  historySearchQuery: '',
  historyTimeRange: 'all',
  historyCustomFrom: null,
  historyCustomTo: null,
  whatsHappeningRange: '1h',
  whatsHappeningCustomFrom: null,
  whatsHappeningCustomTo: null,
  analyticsRange: '7d',
  analyticsCustomFrom: null,
  analyticsCustomTo: null,
  repoSearchTimer: null,
  repoSearchResults: [],
  selectedSearchRepos: {},
  addRepoMode: 'search',
  logViewerJobId: null,
  logRetryTimer: null,
  liveLogJobId: null,
  liveLogTimer: null,
  liveLogPrevText: '',
  liveLogAutoScroll: true,
  liveLogInterval: 3000,
  liveLogStaleCount: 0,
  expandedRunRefreshTimers: {},       // { runId: intervalId }
  theme: 'terminal',
  selectedSidebarRepos: {},
  collapsedWorkflowGroups: {},
  workflowGroupsInitialized: false,
  branchCache: {},
  selectedWorkflowKeys: {},
  customWorkflowTabs: [],             // Pipeline orchestration tabs
  customTabRuntime: {},
  savedReposToRestore: [],
  draggingCustomTabId: null,
  customTabGateResolvers: {},
  openInterStageConfigKey: null,
  globalPipelinePresets: {},
  slackTemplates: [],
  slackConfig: {},
  workflowInputsByKey: {},            // Cached workflow input definitions
};
```

### Function Signatures — Key Backend Functions

```rust
// poller.rs:46-54
pub async fn run_poller(
    client: Arc<GitHubClient>,
    state: SharedDashboardState,
    repos: SharedRepoList,
    ws_tx: broadcast::Sender<WsMessage>,
    poll_interval: Duration,
    shutdown: CancellationToken,
    previous_runs: PreviousRunTracker,
    db: SharedDatabase,
)

// poller.rs:158-166
pub async fn poll_all_repos(
    client: &Arc<GitHubClient>,
    state: &SharedDashboardState,
    repos: &SharedRepoList,
    ws_tx: &broadcast::Sender<WsMessage>,
    previous_runs: &Arc<RwLock<std::collections::HashMap<u64, String>>>,
    is_first_poll: bool,
    db: &SharedDatabase,
)

// poller.rs:268-271
async fn poll_single_repo(
    client: &Arc<GitHubClient>,
    repo_ref: &RepoRef,
) -> Result<RepoState, crate::error::DashboardError>

// ws.rs:23-30
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_tx: web::Data<broadcast::Sender<WsMessage>>,
    state: web::Data<SharedDashboardState>,
    dashboard_token: web::Data<Option<String>>,
    connection_count: web::Data<WsConnectionCount>,
) -> actix_web::Result<HttpResponse>

// database.rs:250
pub fn upsert_runs(&self, repo_owner: &str, repo_name: &str, runs: &[WorkflowRun])
```

### New Dependencies by Tier

| Tier | Crate | Version | Purpose |
|------|-------|---------|---------|
| 1 | `futures` | `0.3` | `stream::iter().buffer_unordered()` for parallel polling (or use existing `futures-util`) |
| 2 | `lru` | `0.12` | LRU cache for ETag entries |
| 3 | `dashmap` | `6` | Concurrent per-repo state map |
| 3 | `arc-swap` | `1` | Lock-free atomic pointer swap for rate limit |
| 3 | `num_cpus` | `1` | Configure actix worker threads |
| 4 | `redis` | `0.27` | Pub/sub for multi-instance broadcast |
| 4 | `sqlx` | `0.8` | Async PostgreSQL with connection pooling |

---

## 8. Verification Checklist

### After Each Tier

- [ ] `cargo test` — 273 tests must pass (62 lib + 106 binary + 105 integration)
- [ ] `cargo clippy` — no new warnings
- [ ] Manual browser test: open dashboard, verify all 6 tabs render data
- [ ] WebSocket reconnect: kill server, restart, confirm auto-reconnect
- [ ] Theme switching: verify all 5 themes still apply correctly

### Tier 1 Specific

- [ ] Parallel polling: log output shows repos polled concurrently (check tracing timestamps)
- [ ] Visible-tab rendering: switch tabs and verify no JavaScript errors in console
- [ ] Search debounce: type quickly in search boxes, verify no UI stutter
- [ ] Database indexes: run `EXPLAIN QUERY PLAN` on analytics queries, verify index usage
- [ ] Duration timers: switch away from Active Runs tab, confirm timers stop (check `State.durationTimers.size`)

### Tier 2 Specific

- [ ] Delta WS: monitor WS traffic (browser DevTools → Network → WS), verify `state_delta` messages are smaller than `state` messages
- [ ] Batch inserts: check SQLite journal size during poll, verify single transaction
- [ ] Circuit breaker: configure a non-existent repo, verify it gets skipped after 3 failures
- [ ] Analytics coalescing: open 50 tabs simultaneously, verify only 1 analytics computation runs
- [ ] LRU ETag: check that frequently-accessed repos always return 304

### Stress Testing

- [ ] Open 50+ browser tabs pointing to dashboard simultaneously
- [ ] Monitor server memory usage over 30 minutes (should plateau, not grow linearly)
- [ ] Check `/api/analytics/summary` response time under 50 concurrent requests
- [ ] Verify WS broadcast latency stays under 100ms for 100 connected clients
- [ ] Rate limit: monitor GitHub API usage stays within budget even with 50 repos

---

## Appendix: Scaling Capacity Summary

| Tier | User Capacity | Effort | Key Changes |
|------|---------------|--------|-------------|
| Current | ~50 | — | Baseline |
| 1 (Quick Wins) | ~75 | Hours | Parallel polling, visible-tab render, debounce, indexes |
| 2 (Architecture) | ~200 | Days | Delta WS, batch DB, circuit breaker, LRU, watchdog |
| 3 (Full Scale) | 500+ | Weeks | Incremental DOM, per-repo locks, log streaming |
| 4 (Enterprise) | 1000+ | Months | Redis pub/sub, PostgreSQL, GitHub App, WS gateway |

### Three Independent Ceilings

1. **GitHub API rate limit** (5,000 req/hr) — scales via: parallel polling, ETag 304s, circuit breaker, GitHub App
2. **Server broadcast bandwidth** — scales via: delta messages, per-repo locks, Redis pub/sub
3. **Frontend rendering** — scales via: visible-tab render, incremental DOM, search debounce
