# Plan: Sidebar Repo Selection + Branches Tab Filtering

**Status: COMPLETED (2026-02-20)**

## Goal
Clicking a repo in the sidebar toggles it as selected/highlighted. The Branches tab only scans selected repos (or all if none selected). Other tabs are unaffected for now but can use this filter later.

## Completed Changes

1. **Frontend State** — Added `selectedSidebarRepos: {}` to State object
2. **CSS** — Added `.repo-item.selected` style (accent left border + highlight) and `cursor: pointer` on `.repo-item-left`
3. **Sidebar Rendering** — `renderSidebar()` adds `onclick` → `toggleSidebarRepo()`, applies `.selected` class, shows "N/total" count
4. **App.toggleSidebarRepo()** — Toggles selection, re-renders sidebar, invalidates branch cache, updates branches tab
5. **Backend** — `StaleBranchesQuery` struct with `repos: Option<String>`, filters `repo_refs` when provided
6. **Frontend fetch** — `fetchStaleBranches()` builds `?repos=` query param from selection, dynamic loading/toast messages
7. **Empty state** — `renderBranches()` shows selected repo names in the "Click Refresh" prompt
