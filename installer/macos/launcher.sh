#!/usr/bin/env bash
# ============================================================================
# GitWorkflowDashboard — macOS App Launcher
# ============================================================================
# This script runs inside the .app bundle. It:
# 1. Reads config from ~/.config/git-workflow-dashboard/config.env (if exists)
# 2. Prompts for GITHUB_TOKEN if not configured
# 3. Launches the dashboard server
# 4. Opens the browser to the dashboard
# ============================================================================

set -euo pipefail

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${APP_DIR}/MacOS/git-workflow-dashboard"
CONFIG_DIR="${HOME}/.config/git-workflow-dashboard"
CONFIG_FILE="${CONFIG_DIR}/config.env"
LOG_FILE="${CONFIG_DIR}/dashboard.log"
PID_FILE="${CONFIG_DIR}/dashboard.pid"
PORT="${SERVER_PORT:-4000}"

# ── Load config ──────────────────────────────────────────────────────────────

if [ -f "$CONFIG_FILE" ]; then
    # shellcheck disable=SC1090
    source "$CONFIG_FILE"
fi

# ── Check for token ──────────────────────────────────────────────────────────

if [ -z "${GITHUB_TOKEN:-}" ]; then
    # Show a dialog to get the token
    TOKEN=$(osascript -e '
        set theToken to text returned of (display dialog "GitWorkflowDashboard needs a GitHub Personal Access Token.\n\nCreate one at github.com/settings/tokens with '\''actions:read'\'' scope.\n\nEnter your GitHub PAT:" default answer "" with title "GitWorkflowDashboard Setup" with icon caution buttons {"Cancel", "Save & Start"} default button "Save & Start")
        return theToken
    ' 2>/dev/null || echo "")

    if [ -z "$TOKEN" ]; then
        osascript -e 'display dialog "No token provided. GitWorkflowDashboard cannot start without a GitHub token." with title "GitWorkflowDashboard" buttons {"OK"} default button "OK" with icon stop' 2>/dev/null || true
        exit 1
    fi

    # Save config
    mkdir -p "$CONFIG_DIR"
    cat > "$CONFIG_FILE" << EOF
# GitWorkflowDashboard configuration
# Generated on $(date)
GITHUB_TOKEN="${TOKEN}"
# GITHUB_REPOS=owner/repo1,owner/repo2
# SERVER_PORT=4000
# POLL_INTERVAL=30
# GITHUB_API_URL=https://api.github.com
# WEBHOOK_SECRET=
EOF
    chmod 600 "$CONFIG_FILE"

    export GITHUB_TOKEN="$TOKEN"

    osascript -e "display notification \"Configuration saved to ${CONFIG_FILE}\" with title \"GitWorkflowDashboard\" subtitle \"Setup Complete\"" 2>/dev/null || true
fi

# ── Stop existing instance ───────────────────────────────────────────────────

if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE" 2>/dev/null || echo "")
    if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
        kill "$OLD_PID" 2>/dev/null || true
        sleep 1
    fi
    rm -f "$PID_FILE"
fi

# ── Build CLI arguments ─────────────────────────────────────────────────────

ARGS=(
    --host "127.0.0.1"
    --port "$PORT"
    --github-token "$GITHUB_TOKEN"
)

if [ -n "${GITHUB_REPOS:-}" ]; then
    ARGS+=(--repos "$GITHUB_REPOS")
fi
if [ -n "${POLL_INTERVAL:-}" ]; then
    ARGS+=(--poll-interval "$POLL_INTERVAL")
fi
if [ -n "${WEBHOOK_SECRET:-}" ]; then
    ARGS+=(--webhook-secret "$WEBHOOK_SECRET")
fi
if [ -n "${GITHUB_API_URL:-}" ]; then
    ARGS+=(--github-api-url "$GITHUB_API_URL")
fi

# ── Start the server ─────────────────────────────────────────────────────────

mkdir -p "$CONFIG_DIR"

export RUST_LOG="${RUST_LOG:-info}"

"$BINARY" "${ARGS[@]}" >> "$LOG_FILE" 2>&1 &
SERVER_PID=$!
echo "$SERVER_PID" > "$PID_FILE"

# Wait for server to be ready (up to 10s)
for i in $(seq 1 20); do
    if curl -s "http://127.0.0.1:${PORT}" > /dev/null 2>&1; then
        break
    fi
    sleep 0.5
done

# ── Open browser ─────────────────────────────────────────────────────────────

open "http://127.0.0.1:${PORT}"

# Show notification
osascript -e "display notification \"Dashboard running at http://127.0.0.1:${PORT}\" with title \"GitWorkflowDashboard\" subtitle \"Server Started\"" 2>/dev/null || true

# Wait for server process (keeps app "running" in dock)
wait "$SERVER_PID" 2>/dev/null || true
