#!/bin/sh
# DXOS installer — https://dxos.dev
# Usage: curl -sSf https://raw.githubusercontent.com/pdaxt/dxos/main/install.sh | sh
set -e

REPO="https://github.com/pdaxt/dxos.git"
INSTALL_DIR="$HOME/.dxos/src"
BINARY_NAME="dxos"

# ─── Colors ───────────────────────────────────────────────────────────
if [ -t 1 ]; then
    BOLD='\033[1m'
    DIM='\033[2m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    RED='\033[0;31m'
    CYAN='\033[0;36m'
    RESET='\033[0m'
else
    BOLD='' DIM='' GREEN='' YELLOW='' RED='' CYAN='' RESET=''
fi

info()  { printf "${CYAN}info${RESET}  %s\n" "$1"; }
ok()    { printf "${GREEN}  ok${RESET}  %s\n" "$1"; }
warn()  { printf "${YELLOW}warn${RESET}  %s\n" "$1"; }
err()   { printf "${RED} err${RESET}  %s\n" "$1" >&2; }
step()  { printf "\n${BOLD}==> %s${RESET}\n" "$1"; }

# ─── Detect OS & Arch ────────────────────────────────────────────────
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  OS="linux" ;;
        Darwin) OS="macos" ;;
        *)      err "Unsupported OS: $OS"; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64|amd64)   ARCH="x86_64" ;;
        aarch64|arm64)  ARCH="aarch64" ;;
        *)              err "Unsupported architecture: $ARCH"; exit 1 ;;
    esac

    ok "Detected $OS/$ARCH"
}

# ─── Check / Install Dependencies ────────────────────────────────────
ensure_rust() {
    if command -v cargo >/dev/null 2>&1; then
        ok "Rust already installed ($(rustc --version 2>/dev/null || echo 'unknown version'))"
        return
    fi

    step "Installing Rust via rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
    ok "Rust installed ($(rustc --version))"
}

ensure_git() {
    if command -v git >/dev/null 2>&1; then
        return
    fi
    err "git is required but not found. Please install git first."
    if [ "$OS" = "macos" ]; then
        err "  Try: xcode-select --install"
    else
        err "  Try: sudo apt install git  OR  sudo dnf install git"
    fi
    exit 1
}

# ─── Clone or Update ─────────────────────────────────────────────────
clone_or_update() {
    if [ -d "$INSTALL_DIR/.git" ]; then
        step "Updating DXOS source"
        cd "$INSTALL_DIR"
        git fetch origin
        git reset --hard origin/main
        ok "Updated to latest"
    else
        step "Cloning DXOS"
        mkdir -p "$(dirname "$INSTALL_DIR")"
        git clone "$REPO" "$INSTALL_DIR"
        ok "Cloned to $INSTALL_DIR"
    fi
}

# ─── Build ────────────────────────────────────────────────────────────
build_binary() {
    step "Building DXOS (release mode)"
    cd "$INSTALL_DIR"
    cargo build --release 2>&1 | tail -5
    ok "Build complete"
}

# ─── Install Binary ──────────────────────────────────────────────────
install_binary() {
    step "Installing binary"
    BUILT="$INSTALL_DIR/target/release/$BINARY_NAME"

    if [ ! -f "$BUILT" ]; then
        # The workspace may produce the binary under the CLI crate name
        for candidate in "$INSTALL_DIR/target/release/dxos-cli" "$INSTALL_DIR/target/release/dxos"; do
            if [ -f "$candidate" ]; then
                BUILT="$candidate"
                break
            fi
        done
    fi

    if [ ! -f "$BUILT" ]; then
        err "Binary not found after build. Check cargo build output above."
        exit 1
    fi

    # Try /usr/local/bin first, fall back to ~/.local/bin
    if [ -w "/usr/local/bin" ]; then
        BIN_DIR="/usr/local/bin"
    elif [ -n "$SUDO_USER" ] || id -u >/dev/null 2>&1 && [ "$(id -u)" = "0" ]; then
        BIN_DIR="/usr/local/bin"
    else
        BIN_DIR="$HOME/.local/bin"
        mkdir -p "$BIN_DIR"
    fi

    cp "$BUILT" "$BIN_DIR/$BINARY_NAME"
    chmod +x "$BIN_DIR/$BINARY_NAME"
    ok "Installed to $BIN_DIR/$BINARY_NAME"

    # Ensure bin dir is in PATH
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *)
            warn "$BIN_DIR is not in your PATH"
            warn "Add this to your shell profile:"
            warn "  export PATH=\"$BIN_DIR:\$PATH\""
            ;;
    esac
}

# ─── Setup (Ollama + Model) ──────────────────────────────────────────
run_setup() {
    step "Running dxos setup"

    # Check if dxos is now available
    if ! command -v "$BINARY_NAME" >/dev/null 2>&1; then
        # Try the direct path
        DXOS_BIN="$BIN_DIR/$BINARY_NAME"
    else
        DXOS_BIN="$BINARY_NAME"
    fi

    if [ -x "$DXOS_BIN" ] || command -v "$DXOS_BIN" >/dev/null 2>&1; then
        "$DXOS_BIN" setup || warn "Setup encountered an issue — you can run 'dxos setup' later"
    else
        warn "Could not find dxos binary to run setup. Run 'dxos setup' manually after adding it to PATH."
    fi
}

# ─── Main ─────────────────────────────────────────────────────────────
main() {
    printf "\n${BOLD}    ____  _  ______  _____${RESET}\n"
    printf "${BOLD}   / __ \\| |/ / __ \\/ ___/${RESET}\n"
    printf "${BOLD}  / / / /|   / / / /\\__ \\ ${RESET}\n"
    printf "${BOLD} / /_/ //   / /_/ /___/ / ${RESET}\n"
    printf "${BOLD}/_____//_/|_\\____//____/  ${RESET}\n"
    printf "${DIM}  The open-source AI agent OS${RESET}\n\n"

    detect_platform
    ensure_git
    ensure_rust
    clone_or_update
    build_binary
    install_binary
    run_setup

    printf "\n${GREEN}${BOLD}DXOS installed successfully!${RESET}\n\n"
    printf "  ${BOLD}Get started:${RESET}\n"
    printf "    ${CYAN}dxos chat${RESET}           Start an interactive session\n"
    printf "    ${CYAN}dxos run \"fix bug\"${RESET}  Run a one-shot task\n"
    printf "    ${CYAN}dxos setup${RESET}          Configure models\n"
    printf "    ${CYAN}dxos --help${RESET}         See all commands\n\n"
    printf "  ${DIM}Docs:   https://dxos.dev${RESET}\n"
    printf "  ${DIM}GitHub: https://github.com/pdaxt/dxos${RESET}\n\n"
}

main
