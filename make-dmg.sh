#!/usr/bin/env bash
# ============================================================================
# GitWorkflowDashboard — macOS DMG Installer Builder
# ============================================================================
# Usage:
#   ./make-dmg.sh                Build DMG for current architecture
#   ./make-dmg.sh --universal    Build universal (x86_64 + arm64) binary
#   ./make-dmg.sh --clean        Remove build artifacts
#   ./make-dmg.sh --help         Show help
# ============================================================================

set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALLER_DIR="${PROJECT_DIR}/installer/macos"
BUILD_DIR="${PROJECT_DIR}/target/dmg-build"

APP_NAME="GitWorkflowDashboard"
BUNDLE_NAME="${APP_NAME}.app"
DMG_VOLUME_NAME="${APP_NAME}"
BINARY_NAME="git-workflow-dashboard"
VERSION=$(grep '^version' "${PROJECT_DIR}/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(uname -m)

# ── Colors ───────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# ── Functions ────────────────────────────────────────────────────────────────

print_help() {
    echo ""
    echo -e "${CYAN}${APP_NAME}${NC} — macOS DMG Installer Builder"
    echo ""
    echo "Usage:"
    echo "  ./make-dmg.sh                Build DMG for current architecture (${ARCH})"
    echo "  ./make-dmg.sh --universal    Build universal binary (x86_64 + arm64)"
    echo "  ./make-dmg.sh --clean        Remove build artifacts"
    echo "  ./make-dmg.sh --help         Show help"
    echo ""
    echo "Output:"
    echo "  ${APP_NAME}-v${VERSION}-<arch>.dmg"
    echo ""
}

print_step() {
    echo ""
    echo -e "${BOLD}${GREEN}▸ $1${NC}"
}

print_info() {
    echo -e "  ${CYAN}$1${NC}"
}

check_deps() {
    local missing=()
    for cmd in cargo sips iconutil hdiutil python3; do
        if ! command -v "$cmd" > /dev/null 2>&1; then
            missing+=("$cmd")
        fi
    done
    if [ ${#missing[@]} -gt 0 ]; then
        echo -e "${RED}Missing required tools: ${missing[*]}${NC}"
        exit 1
    fi
}

build_binary() {
    local target_flag=""
    local binary_path=""

    if [ "${UNIVERSAL:-false}" = "true" ]; then
        print_step "Building universal binary (x86_64 + arm64)"

        # Check if both targets are installed
        if ! rustup target list --installed | grep -q "x86_64-apple-darwin"; then
            print_info "Adding x86_64-apple-darwin target..."
            rustup target add x86_64-apple-darwin
        fi
        if ! rustup target list --installed | grep -q "aarch64-apple-darwin"; then
            print_info "Adding aarch64-apple-darwin target..."
            rustup target add aarch64-apple-darwin
        fi

        print_info "Building x86_64..."
        cargo build --release --target x86_64-apple-darwin

        print_info "Building arm64..."
        cargo build --release --target aarch64-apple-darwin

        print_info "Creating universal binary with lipo..."
        mkdir -p "${BUILD_DIR}"
        lipo -create \
            "${PROJECT_DIR}/target/x86_64-apple-darwin/release/${BINARY_NAME}" \
            "${PROJECT_DIR}/target/aarch64-apple-darwin/release/${BINARY_NAME}" \
            -output "${BUILD_DIR}/${BINARY_NAME}"

        binary_path="${BUILD_DIR}/${BINARY_NAME}"
        ARCH="universal"
    else
        print_step "Building release binary (${ARCH})"
        cargo build --release
        binary_path="${PROJECT_DIR}/target/release/${BINARY_NAME}"
    fi

    # Verify binary
    local file_info
    file_info=$(file "$binary_path")
    local binary_size
    binary_size=$(du -h "$binary_path" | cut -f1 | tr -d ' ')
    print_info "Binary: ${binary_path}"
    print_info "Size: ${binary_size}"
    print_info "Type: ${file_info##*: }"

    echo "$binary_path"
}

create_icon() {
    print_step "Creating app icon"
    bash "${INSTALLER_DIR}/create-icon.sh"
}

create_app_bundle() {
    local binary_path="$1"

    print_step "Creating ${BUNDLE_NAME}"

    local app_dir="${BUILD_DIR}/${BUNDLE_NAME}"
    rm -rf "$app_dir"

    # Create bundle structure
    mkdir -p "${app_dir}/Contents/MacOS"
    mkdir -p "${app_dir}/Contents/Resources"

    # Copy binary
    cp "$binary_path" "${app_dir}/Contents/MacOS/${BINARY_NAME}"
    chmod 755 "${app_dir}/Contents/MacOS/${BINARY_NAME}"

    # Copy and install launcher
    cp "${INSTALLER_DIR}/launcher.sh" "${app_dir}/Contents/MacOS/launcher"
    chmod 755 "${app_dir}/Contents/MacOS/launcher"

    # Copy Info.plist with version substitution
    sed "s/__VERSION__/${VERSION}/g" "${INSTALLER_DIR}/Info.plist" > "${app_dir}/Contents/Info.plist"

    # Copy icon
    if [ -f "${INSTALLER_DIR}/AppIcon.icns" ]; then
        cp "${INSTALLER_DIR}/AppIcon.icns" "${app_dir}/Contents/Resources/AppIcon.icns"
    fi

    print_info "Bundle created at ${app_dir}"
    echo "$app_dir"
}

create_dmg() {
    local app_dir="$1"
    local dmg_name="${APP_NAME}-v${VERSION}-${ARCH}.dmg"
    local dmg_path="${PROJECT_DIR}/${dmg_name}"
    local dmg_temp="${BUILD_DIR}/temp.dmg"

    print_step "Creating DMG: ${dmg_name}"

    # Remove old DMG if exists
    rm -f "$dmg_path"
    rm -f "$dmg_temp"

    # Create staging directory
    local staging="${BUILD_DIR}/dmg-staging"
    rm -rf "$staging"
    mkdir -p "$staging"

    # Copy app bundle
    cp -R "$app_dir" "$staging/"

    # Create Applications symlink
    ln -s /Applications "$staging/Applications"

    # Create install script that handles upgrades
    cat > "${staging}/Install.command" << 'INSTALLEOF'
#!/usr/bin/env bash
# ============================================================================
# GitWorkflowDashboard — Installer
# Stops any running instance, removes old app, installs new one.
# ============================================================================
set -euo pipefail

APP_NAME="GitWorkflowDashboard"
BUNDLE="${APP_NAME}.app"
INSTALL_DIR="/Applications"
CONFIG_DIR="${HOME}/.config/git-workflow-dashboard"
PID_FILE="${CONFIG_DIR}/dashboard.pid"
DMG_DIR="$(cd "$(dirname "$0")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  ${APP_NAME} — Installer${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════${NC}"
echo ""

# Stop running instance
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE" 2>/dev/null || echo "")
    if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
        echo -e "${YELLOW}▸ Stopping running instance (PID ${OLD_PID})...${NC}"
        kill "$OLD_PID" 2>/dev/null || true
        sleep 1
    fi
    rm -f "$PID_FILE"
fi

# Also check for any process on default port
if lsof -ti :4000 > /dev/null 2>&1; then
    echo -e "${YELLOW}▸ Stopping process on port 4000...${NC}"
    lsof -ti :4000 | xargs kill 2>/dev/null || true
    sleep 1
fi

# Remove old installation
if [ -d "${INSTALL_DIR}/${BUNDLE}" ]; then
    echo -e "${YELLOW}▸ Removing old installation...${NC}"
    rm -rf "${INSTALL_DIR}/${BUNDLE}"
fi

# Install new version
echo -e "${GREEN}▸ Installing ${BUNDLE} to ${INSTALL_DIR}...${NC}"
cp -R "${DMG_DIR}/${BUNDLE}" "${INSTALL_DIR}/"

echo ""
echo -e "${GREEN}✓ Installation complete!${NC}"
echo ""
echo -e "  Launch from ${CYAN}Applications → ${APP_NAME}${NC}"
echo -e "  Or run: ${CYAN}open -a ${APP_NAME}${NC}"
echo ""

# Ask to launch now
read -p "Launch now? [Y/n] " -n 1 -r
echo ""
if [[ ! $REPLY =~ ^[Nn]$ ]]; then
    open -a "$APP_NAME"
fi
INSTALLEOF
    chmod +x "${staging}/Install.command"

    # Create uninstall script
    cat > "${staging}/Uninstall.command" << 'UNINSTALLEOF'
#!/usr/bin/env bash
# ============================================================================
# GitWorkflowDashboard — Uninstaller
# Stops any running instance, removes app and optionally config.
# ============================================================================
set -euo pipefail

APP_NAME="GitWorkflowDashboard"
BUNDLE="${APP_NAME}.app"
INSTALL_DIR="/Applications"
CONFIG_DIR="${HOME}/.config/git-workflow-dashboard"
PID_FILE="${CONFIG_DIR}/dashboard.pid"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  ${APP_NAME} — Uninstaller${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════${NC}"
echo ""

# Stop running instance
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE" 2>/dev/null || echo "")
    if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
        echo -e "${YELLOW}▸ Stopping running instance (PID ${OLD_PID})...${NC}"
        kill "$OLD_PID" 2>/dev/null || true
        sleep 1
    fi
    rm -f "$PID_FILE"
fi

if lsof -ti :4000 > /dev/null 2>&1; then
    echo -e "${YELLOW}▸ Stopping process on port 4000...${NC}"
    lsof -ti :4000 | xargs kill 2>/dev/null || true
    sleep 1
fi

# Remove app
if [ -d "${INSTALL_DIR}/${BUNDLE}" ]; then
    echo -e "${YELLOW}▸ Removing ${BUNDLE} from ${INSTALL_DIR}...${NC}"
    rm -rf "${INSTALL_DIR}/${BUNDLE}"
    echo -e "${GREEN}✓ Application removed.${NC}"
else
    echo -e "${YELLOW}  ${BUNDLE} not found in ${INSTALL_DIR}.${NC}"
fi

# Ask about config
echo ""
if [ -d "$CONFIG_DIR" ]; then
    read -p "Remove configuration and logs (~/.config/git-workflow-dashboard)? [y/N] " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$CONFIG_DIR"
        echo -e "${GREEN}✓ Configuration removed.${NC}"
    else
        echo -e "  Config kept at: ${CYAN}${CONFIG_DIR}${NC}"
    fi
fi

echo ""
echo -e "${GREEN}✓ Uninstall complete.${NC}"
echo ""
UNINSTALLEOF
    chmod +x "${staging}/Uninstall.command"

    # Create README in DMG
    cat > "${staging}/README.txt" << 'READMEEOF'
GitWorkflowDashboard
====================
GitHub Actions Monitoring Dashboard

INSTALL / UPGRADE
  Option A: Drag GitWorkflowDashboard.app to the Applications folder.
  Option B: Double-click Install.command (stops old, removes old, installs new).

FIRST RUN
  1. Open GitWorkflowDashboard from Applications
  2. Enter your GitHub Personal Access Token when prompted
     (Create one at github.com/settings/tokens with 'actions:read' scope)
     Or set GITHUB_TOKEN in your ~/.zshrc to skip the prompt.
  3. The dashboard opens in your browser at http://127.0.0.1:4000

CONFIGURATION
  Config is stored at: ~/.config/git-workflow-dashboard/config.env
  Edit this file to set repos, port, poll interval, etc.

  Available settings:
    GITHUB_TOKEN=ghp_...          (required) Your GitHub PAT
    GITHUB_REPOS=owner/repo       Comma-separated repos to monitor
    SERVER_PORT=4000              Dashboard port
    POLL_INTERVAL=30              Seconds between polls
    GITHUB_API_URL=https://...    For GitHub Enterprise
    WEBHOOK_SECRET=...            For webhook verification

UNINSTALL
  Option A: Move GitWorkflowDashboard.app to Trash, then optionally:
            rm -rf ~/.config/git-workflow-dashboard
  Option B: Double-click Uninstall.command (handles everything).

LOGS
  View logs at: ~/.config/git-workflow-dashboard/dashboard.log
READMEEOF

    # Calculate DMG size (app size + 10MB headroom)
    local app_size_kb
    app_size_kb=$(du -sk "$staging" | cut -f1)
    local dmg_size_kb=$((app_size_kb + 10240))

    # Detach any existing mounts with the same volume name
    for vol in /Volumes/${DMG_VOLUME_NAME}*; do
        if [ -d "$vol" ]; then
            hdiutil detach "$vol" -force > /dev/null 2>&1 || true
        fi
    done

    # Create temporary DMG
    hdiutil create \
        -srcfolder "$staging" \
        -volname "$DMG_VOLUME_NAME" \
        -fs HFS+ \
        -fsargs "-c c=64,a=16,e=16" \
        -format UDRW \
        -size "${dmg_size_kb}k" \
        "$dmg_temp" \
        > /dev/null

    # Mount the DMG to customize it
    local mount_output
    mount_output=$(hdiutil attach -readwrite -noverify -noautoopen "$dmg_temp")
    local mount_point="/Volumes/${DMG_VOLUME_NAME}"

    if [ -d "$mount_point" ]; then
        # Set DMG window properties via AppleScript
        osascript << APPLESCRIPT > /dev/null 2>&1 || true
tell application "Finder"
    tell disk "${DMG_VOLUME_NAME}"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set the bounds of container window to {400, 80, 960, 520}
        set viewOptions to the icon view options of container window
        set arrangement of viewOptions to not arranged
        set icon size of viewOptions to 80
        set position of item "${BUNDLE_NAME}" of container window to {140, 150}
        set position of item "Applications" of container window to {420, 150}
        set position of item "Install.command" of container window to {140, 320}
        set position of item "Uninstall.command" of container window to {280, 320}
        set position of item "README.txt" of container window to {420, 320}
        close
        open
        update without registering applications
        delay 1
    end tell
end tell
APPLESCRIPT

        sync
        sleep 1

        # Unmount
        hdiutil detach "$mount_point" -force > /dev/null 2>&1 || true
    fi

    # Convert to compressed DMG
    hdiutil convert "$dmg_temp" \
        -format UDZO \
        -imagekey zlib-level=9 \
        -o "$dmg_path" \
        > /dev/null

    rm -f "$dmg_temp"

    # Show result
    local dmg_size
    dmg_size=$(du -h "$dmg_path" | cut -f1 | tr -d ' ')
    print_info "DMG: ${dmg_path}"
    print_info "Size: ${dmg_size}"

    echo "$dmg_path"
}

clean() {
    print_step "Cleaning build artifacts"
    rm -rf "${BUILD_DIR}"
    rm -f "${INSTALLER_DIR}/AppIcon.icns"
    rm -rf "${INSTALLER_DIR}/AppIcon.iconset"
    rm -f "${PROJECT_DIR}/${APP_NAME}"-*.dmg
    print_info "Cleaned."
}

# ── Main ─────────────────────────────────────────────────────────────────────

UNIVERSAL=false

case "${1:-}" in
    --universal)
        UNIVERSAL=true
        ;;
    --clean)
        clean
        exit 0
        ;;
    --help|-h)
        print_help
        exit 0
        ;;
    "")
        ;;
    *)
        echo -e "${RED}Unknown option: $1${NC}"
        print_help
        exit 1
        ;;
esac

# Update ARCH before it's used in filenames and display
if [ "${UNIVERSAL}" = "true" ]; then
    ARCH="universal"
fi

echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════${NC}"
echo -e "${BOLD}${CYAN}  ${APP_NAME} — DMG Installer Builder${NC}"
echo -e "${BOLD}${CYAN}  Version: ${VERSION}  Architecture: ${ARCH}${NC}"
echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════${NC}"

check_deps

mkdir -p "${BUILD_DIR}"

# Step 1: Build binary
BINARY_PATH=$(build_binary | tail -1)

# Step 2: Create icon
create_icon

# Step 3: Create .app bundle
APP_DIR=$(create_app_bundle "$BINARY_PATH" | tail -1)

# Step 4: Create DMG
DMG_PATH=$(create_dmg "$APP_DIR" | tail -1)

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}${GREEN}═══════════════════════════════════════════════════${NC}"
echo -e "${BOLD}${GREEN}  ✓ DMG installer created successfully!${NC}"
echo -e "${BOLD}${GREEN}═══════════════════════════════════════════════════${NC}"
echo ""
echo -e "  ${CYAN}File:${NC} ${DMG_PATH}"
echo -e "  ${CYAN}Size:${NC} $(du -h "$DMG_PATH" | cut -f1 | tr -d ' ')"
echo ""
echo -e "  ${YELLOW}To install:${NC}"
echo "  1. Open the DMG"
echo "  2. Drag GitWorkflowDashboard to Applications"
echo "  3. Launch from Applications folder"
echo ""
echo -e "  ${YELLOW}Note:${NC} On first launch, macOS may show a security warning."
echo "  Go to System Preferences → Privacy & Security → Open Anyway"
echo ""
