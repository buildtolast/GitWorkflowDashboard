# Plan: Branches Feature

**Status: DISABLED (2026-02-20)**

Branches tab and all branch API routes temporarily disabled. Code preserved with `#[allow(dead_code)]` for future re-enablement.

## What was disabled
- Frontend: Branches tab button (HTML-commented out)
- Backend: `/api/branches/stale`, `/api/branches/{o}/{r}`, `DELETE /api/branches/{o}/{r}/{branch}` routes (commented out in main.rs)
- All branch-related code kept intact with `#[allow(dead_code)]` annotations

## To re-enable
1. Uncomment tab button in `src/frontend/index.html` (search "Branches tab disabled")
2. Uncomment routes in `src/main.rs` (search "will revisit later")
3. Remove `#[allow(dead_code)]` from: `handlers/mod.rs`, `config.rs`, `github/client.rs`, `handlers/workflows.rs`, `models.rs`
