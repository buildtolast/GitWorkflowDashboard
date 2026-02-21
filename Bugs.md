# Bug Tracker

## Fixed

### BUG-003: Duplicate `filterBranches` method in frontend
- **Severity:** Medium
- **Commit:** `8dc4a8d` — Fix duplicate filterBranches method in frontend
- **Root cause:** The `App` object had two methods named `filterBranches` — one for the Branches tab search (line 4889) and one for the dispatch modal branch filter (line 5160). In JavaScript, the second definition silently overwrites the first, so the Branches tab search called the dispatch modal logic instead.
- **Impact:** Branches tab search filter was broken (would fail silently or manipulate the wrong DOM element). Dispatch modal branch filter worked fine since it was the surviving definition.
- **Fix:** Renamed the dispatch modal version to `filterDispatchBranches` and updated its HTML `oninput` caller.

### BUG-002: Job logs route ordering causes intermittent "repo not found" error
- **Severity:** High
- **Commit:** `1c80231` — Fix job logs route ordering to prevent repo-not-found error
- **Root cause:** The route `/runs/{owner}/{repo}/jobs/{job_id}/logs` was registered **after** `/runs/{owner}/{repo}/{run_id}` in `main.rs`. When a log request like `/api/runs/owner/repo/jobs/12345/logs` arrived, actix-web matched it against the `{run_id}` wildcard first, capturing `"jobs"` as `run_id`. Since `"jobs"` cannot parse as `u64`, the handler returned an error. The request sometimes succeeded on retry or after a delay.
- **Impact:** "View Logs" button in Active Runs showed "Repository not found" error on first click, but worked after retrying.
- **Fix:** Moved the job logs route before the `{run_id}` wildcard so the more specific path matches first.

### BUG-001: Universal DMG filename shows host architecture instead of "universal"
- **Severity:** Low
- **Commit:** `f7b191c` — Fix universal DMG filename to show 'universal' instead of host arch
- **Root cause:** The DMG build script used the host architecture in the filename even when building a universal (fat) binary.
- **Impact:** Cosmetic — the DMG file was named incorrectly but the binary inside was correct.
- **Fix:** Updated `make-dmg.sh` to use "universal" in the filename when `--universal` flag is passed.

## Investigated (Not Bugs)

### False positive: Route path parameter mismatch in `get_job_log`
- **Reported:** Handler uses `Path<(String, String, u64)>` but route has 5 segments
- **Verdict:** Not a bug. actix-web only extracts named `{params}`, not literal segments like `jobs` and `logs`. The route `/runs/{owner}/{repo}/jobs/{job_id}/logs` has exactly 3 named parameters matching the 3-tuple.

### False positive: Off-by-one panic in timestamp stripping (`parse_step_logs`)
- **Reported:** `line[29..]` could panic when `line.len() == 29`
- **Verdict:** Not a bug. Rust allows `s[n..]` where `n == s.len()` — returns an empty string slice without panicking. Verified with rustc.

### False positive: `is_none_or()` Rust version compatibility
- **Reported:** Method may not exist in older Rust versions
- **Verdict:** Not a bug. `is_none_or()` is stable since Rust 1.82. Project runs on Rust 1.93 with no MSRV set.
