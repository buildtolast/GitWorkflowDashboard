#!/usr/bin/env bash
# ============================================================================
# GitWorkflowDashboard — run.sh
# ============================================================================
# Usage:
#   ./run.sh                      Run from source with cargo (default)
#   ./run.sh --docker             Run in Docker
#   ./run.sh --dmg                Build macOS DMG installer
#   ./run.sh --build              Clean rebuild + run tests + build DMG
#   ./run.sh --clean              Remove all build artifacts
#   ./run.sh --restart            Rebuild + restart (Docker or cargo)
#   ./run.sh --stop               Stop whatever was started
#   ./run.sh --help               Show this help
# ============================================================================

set -euo pipefail

# ── Configuration (env vars with defaults) ──────────────────────────────────
export GITHUB_TOKEN="${GITHUB_TOKEN:-}"
export GITHUB_REPOS="${GITHUB_REPOS:-}"
export SERVER_HOST="${SERVER_HOST:-127.0.0.1}"
export SERVER_PORT="${SERVER_PORT:-4000}"
export POLL_INTERVAL="${POLL_INTERVAL:-30}"
export WEBHOOK_SECRET="${WEBHOOK_SECRET:-}"
export GITHUB_API_URL="${GITHUB_API_URL:-https://api.github.com}"
export RUST_LOG="${RUST_LOG:-info}"
export LOG_FORMAT="${LOG_FORMAT:-}"

APP_NAME="git-workflow-dashboard"
DOCKER_CONTAINER="git-workflow-dashboard"

# ── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ── Functions ───────────────────────────────────────────────────────────────

print_help() {
    echo ""
    echo -e "${CYAN}GitWorkflowDashboard${NC} — GitHub Actions monitoring dashboard"
    echo ""
    echo "Usage:"
    echo "  ./run.sh                Run from source with cargo (default)"
    echo "  ./run.sh --docker       Run in Docker"
    echo "  ./run.sh --dmg          Build macOS DMG installer"
    echo "  ./run.sh --build        Clean rebuild + run tests + build DMG"
    echo "  ./run.sh --clean        Remove all build artifacts"
    echo "  ./run.sh --restart      Rebuild + restart (Docker or cargo)"
    echo "  ./run.sh --stop         Stop whatever was started"
    echo "  ./run.sh --help         Show this help"
    echo ""
    echo "Environment Variables:"
    echo "  GITHUB_TOKEN            GitHub PAT (required)"
    echo "  GITHUB_REPOS            Comma-separated repos (e.g. owner/repo1,owner/repo2)"
    echo "  SERVER_HOST             Server host (default: 127.0.0.1)"
    echo "  SERVER_PORT             Server port (default: 4000)"
    echo "  POLL_INTERVAL           Poll interval in seconds (default: 30)"
    echo "  WEBHOOK_SECRET          Webhook secret (optional)"
    echo "  GITHUB_API_URL          API URL (default: https://api.github.com)"
    echo "  RUST_LOG                Log level (default: info)"
    echo "  LOG_FORMAT              Set to 'json' for JSON logs"
    echo ""
}

print_config() {
    echo -e "${CYAN}Configuration:${NC}"
    echo "  GITHUB_TOKEN:    ${GITHUB_TOKEN:+***set***}"
    echo "  GITHUB_REPOS:    ${GITHUB_REPOS:-<none>}"
    echo "  SERVER_HOST:     ${SERVER_HOST}"
    echo "  SERVER_PORT:     ${SERVER_PORT}"
    echo "  POLL_INTERVAL:   ${POLL_INTERVAL}s"
    echo "  GITHUB_API_URL:  ${GITHUB_API_URL}"
    echo "  RUST_LOG:        ${RUST_LOG}"
    echo ""
}

check_token() {
    if [ -z "$GITHUB_TOKEN" ]; then
        echo -e "${RED}Error: GITHUB_TOKEN is required${NC}"
        echo ""
        echo "Set it via environment variable:"
        echo "  export GITHUB_TOKEN=ghp_your_token_here"
        echo ""
        echo "Or pass inline:"
        echo "  GITHUB_TOKEN=ghp_xxx ./run.sh"
        echo ""
        exit 1
    fi
}

run_cargo() {
    check_token
    print_config

    echo -e "${GREEN}Starting from source with cargo...${NC}"
    echo -e "Dashboard will be at: ${CYAN}http://${SERVER_HOST}:${SERVER_PORT}${NC}"
    echo ""

    ARGS="--host ${SERVER_HOST} --port ${SERVER_PORT} --poll-interval ${POLL_INTERVAL}"
    ARGS="${ARGS} --github-token ${GITHUB_TOKEN}"
    ARGS="${ARGS} --github-api-url ${GITHUB_API_URL}"

    if [ -n "${GITHUB_REPOS}" ]; then
        ARGS="${ARGS} --repos ${GITHUB_REPOS}"
    fi
    if [ -n "${WEBHOOK_SECRET}" ]; then
        ARGS="${ARGS} --webhook-secret ${WEBHOOK_SECRET}"
    fi

    # shellcheck disable=SC2086
    cargo run -- ${ARGS}
}

run_docker() {
    check_token
    print_config

    echo -e "${GREEN}Starting in Docker...${NC}"
    echo -e "Dashboard will be at: ${CYAN}http://localhost:${SERVER_PORT}${NC}"
    echo ""

    docker compose up -d --build
    echo ""
    echo -e "${GREEN}Docker container started.${NC}"
    echo -e "View logs: ${YELLOW}docker logs -f ${DOCKER_CONTAINER}${NC}"
}

run_stop() {
    echo -e "${YELLOW}Stopping GitWorkflowDashboard...${NC}"

    # Check Docker
    if docker ps -q -f "name=${DOCKER_CONTAINER}" 2>/dev/null | grep -q .; then
        echo "Stopping Docker container..."
        docker compose down
        echo -e "${GREEN}Docker container stopped.${NC}"
        return
    fi

    # Check cargo process
    if pgrep -f "target/debug/${APP_NAME}" > /dev/null 2>&1; then
        echo "Stopping cargo process..."
        pkill -f "target/debug/${APP_NAME}" || true
        echo -e "${GREEN}Process stopped.${NC}"
        return
    fi
    if pgrep -f "target/release/${APP_NAME}" > /dev/null 2>&1; then
        echo "Stopping cargo process..."
        pkill -f "target/release/${APP_NAME}" || true
        echo -e "${GREEN}Process stopped.${NC}"
        return
    fi

    # Check port
    if lsof -ti ":${SERVER_PORT}" > /dev/null 2>&1; then
        echo "Killing process on port ${SERVER_PORT}..."
        lsof -ti ":${SERVER_PORT}" | xargs kill -9 2>/dev/null || true
        echo -e "${GREEN}Process killed.${NC}"
        return
    fi

    echo -e "${YELLOW}No running instance found.${NC}"
}

run_restart() {
    echo -e "${CYAN}═══════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}  GitWorkflowDashboard — Restart${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════${NC}"
    echo ""

    # Detect running mode and restart accordingly
    if docker ps -q -f "name=${DOCKER_CONTAINER}" 2>/dev/null | grep -q .; then
        echo -e "${YELLOW}▸ Docker container detected — rebuilding image...${NC}"
        docker compose up -d --build
        echo -e "${GREEN}✓ Docker container rebuilt and restarted.${NC}"
    elif pgrep -f "target/(debug|release)/${APP_NAME}" > /dev/null 2>&1; then
        echo -e "${YELLOW}▸ Cargo process detected — stopping...${NC}"
        pkill -f "target/(debug|release)/${APP_NAME}" || true
        sleep 1
        echo -e "${YELLOW}▸ Rebuilding and starting...${NC}"
        run_cargo
    else
        echo -e "${YELLOW}No running instance found. Starting fresh...${NC}"
        run_cargo
    fi
}

# ── Main ────────────────────────────────────────────────────────────────────

case "${1:-}" in
    --docker)
        run_docker
        ;;
    --dmg)
        echo -e "${GREEN}Building macOS DMG installer...${NC}"
        SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
        exec "${SCRIPT_DIR}/make-dmg.sh" "${2:-}"
        ;;
    --build)
        SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
        echo -e "${CYAN}═══════════════════════════════════════════════════${NC}"
        echo -e "${CYAN}  GitWorkflowDashboard — Full Build${NC}"
        echo -e "${CYAN}═══════════════════════════════════════════════════${NC}"
        echo ""

        echo -e "${YELLOW}▸ Cleaning build artifacts...${NC}"
        cargo clean
        "${SCRIPT_DIR}/make-dmg.sh" --clean 2>/dev/null || true
        echo -e "${GREEN}✓ Clean complete.${NC}"
        echo ""

        echo -e "${YELLOW}▸ Running tests...${NC}"
        if cargo test; then
            echo -e "${GREEN}✓ All tests passed.${NC}"
        else
            echo -e "${RED}✗ Tests failed. Aborting build.${NC}"
            exit 1
        fi
        echo ""

        echo -e "${YELLOW}▸ Building DMG installer...${NC}"
        "${SCRIPT_DIR}/make-dmg.sh" "${2:-}"
        ;;
    --clean)
        SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
        echo -e "${YELLOW}Cleaning all build artifacts...${NC}"
        cargo clean
        "${SCRIPT_DIR}/make-dmg.sh" --clean 2>/dev/null || true
        echo -e "${GREEN}✓ All clean.${NC}"
        ;;
    --restart)
        run_restart
        ;;
    --stop)
        run_stop
        ;;
    --help|-h)
        print_help
        ;;
    "")
        run_cargo
        ;;
    *)
        echo -e "${RED}Unknown option: $1${NC}"
        print_help
        exit 1
        ;;
esac
