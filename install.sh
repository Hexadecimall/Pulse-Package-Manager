#!/usr/bin/env bash
#
# Interactive installer for Pulse. Downloads a prebuilt release — no building.
#
#   ./install.sh [stable|beta|dev] [--yes]
#
# With no --yes it shows a short setup menu. --yes (or a non-interactive shell)
# takes the defaults: a system install with the setuid helper when run as root,
# a user install in ~/.local/bin otherwise.
#
set -euo pipefail

OWNER="Hexadecimall"
REPO="Pulse-Package-Manager"

CHANNEL="stable"
ASSUME_YES=0
for arg in "$@"; do
    case "$arg" in
        stable|beta|dev) CHANNEL="$arg" ;;
        --yes|--defaults) ASSUME_YES=1 ;;
        *) echo "usage: install.sh [stable|beta|dev] [--yes]" >&2; exit 1 ;;
    esac
done

# --- platform ----------------------------------------------------------------
case "$(uname -s)" in
    Darwin) OS="macos" ;;
    Linux) OS="linux" ;;
    *) echo "unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
    arm64|aarch64) ARCH="arm64" ;;
    x86_64|amd64) ARCH="x64" ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac
ASSET="pulse-${OS}-${ARCH}.tar.gz"

# Per-OS system install directory (user override via PULSE_PREFIX).
if [ -n "${PULSE_PREFIX:-}" ]; then
    SYS_DIR="$PULSE_PREFIX/bin"
elif [ "$OS" = "linux" ]; then
    SYS_DIR="/usr/bin"
else
    SYS_DIR="/opt/pulse/bin"
fi

# --- settings (defaults, then the menu may change them) ----------------------
# INSTALL_TYPE: global | user | user+helper
if [ "$(id -u)" -eq 0 ]; then
    INSTALL_TYPE="global"
else
    INSTALL_TYPE="user"
fi
LOCATION=""   # empty => the default for the chosen type

have_tty() { [ -e /dev/tty ]; }

if [ "$ASSUME_YES" -eq 0 ] && have_tty; then
    echo "Welcome to setup for pulse!"
    echo "Press enter to go ahead and use default settings and press a number to modify settings!"
    echo
    echo "[1] User-mode settings"
    echo "[2] Location"
    printf '> '
    read -r choice < /dev/tty || choice=""

    case "$choice" in
        1)
            echo
            echo "[1] Install helper anyway"
            echo "[2] Install globally"
            echo "[3] Install usermode"
            printf '> '
            read -r sub < /dev/tty || sub=""
            case "$sub" in
                1) INSTALL_TYPE="user+helper" ;;
                2) INSTALL_TYPE="global" ;;
                3) INSTALL_TYPE="user" ;;
                *) echo "unrecognized choice; using defaults" ;;
            esac
            ;;
        2)
            printf '[location]: '
            read -r LOCATION < /dev/tty || LOCATION=""
            ;;
        "" ) : ;;  # enter => defaults
        *) echo "unrecognized choice; using defaults" ;;
    esac
fi

# Resolve the binary destination directory.
if [ -n "$LOCATION" ]; then
    BIN_DIR="$LOCATION"
elif [ "$INSTALL_TYPE" = "global" ]; then
    BIN_DIR="$SYS_DIR"
else
    BIN_DIR="$HOME/.local/bin"
fi
# The helper always lands in the system directory (root-owned, setuid).
HELPER_DIR="$SYS_DIR"
WANT_HELPER=0
[ "$INSTALL_TYPE" = "global" ] && WANT_HELPER=1
[ "$INSTALL_TYPE" = "user+helper" ] && WANT_HELPER=1

# --- download ----------------------------------------------------------------
case "$CHANNEL" in
    stable) URL="https://github.com/$OWNER/$REPO/releases/latest/download/$ASSET" ;;
    dev) URL="https://github.com/$OWNER/$REPO/releases/download/dev/$ASSET" ;;
    beta)
        TAG="$(curl -fsSL "https://api.github.com/repos/$OWNER/$REPO/releases?per_page=30" \
            | grep -o '"tag_name": *"[^"]*beta[^"]*"' | head -1 \
            | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
        [ -n "$TAG" ] || { echo "no beta release is available yet" >&2; exit 1; }
        URL="https://github.com/$OWNER/$REPO/releases/download/$TAG/$ASSET"
        ;;
esac

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
echo
echo "pulse: downloading $ASSET ($CHANNEL)..."
curl -fSL "$URL" -o "$TMP/$ASSET"
tar -C "$TMP" -xzf "$TMP/$ASSET"
[ -f "$TMP/pulse" ] || { echo "release archive did not contain 'pulse'" >&2; exit 1; }
chmod +x "$TMP/pulse"

# install <mode> <owner-or-empty> <src> <dest-dir> — uses sudo when the target
# isn't writable; if root is needed but unavailable, fails the caller.
place() {
    local mode="$1" owner="$2" src="$3" dir="$4"
    if [ -w "$dir" ] || { [ ! -e "$dir" ] && [ -w "$(dirname "$dir")" ]; }; then
        install -d "$dir"
        install -m "$mode" "$src" "$dir/$(basename "$src")"
    elif command -v sudo >/dev/null 2>&1; then
        sudo install -d "$dir"
        if [ -n "$owner" ]; then
            sudo install -o "$owner" -m "$mode" "$src" "$dir/$(basename "$src")"
        else
            sudo install -m "$mode" "$src" "$dir/$(basename "$src")"
        fi
    else
        return 1
    fi
}

record_mode() {
    local mode_name="$1" home
    home="$(eval echo "~${SUDO_USER:-$USER}")"
    install -d "$home/.pulse"
    printf 'install_mode = "%s"\n' "$mode_name" > "$home/.pulse/config"
}

# --- install pulse -----------------------------------------------------------
if [ "$INSTALL_TYPE" = "global" ]; then
    if ! place 0755 root "$TMP/pulse" "$BIN_DIR"; then
        echo "pulse: no root available for a global install — falling back to ~/.local/bin" >&2
        INSTALL_TYPE="user"; WANT_HELPER=0; BIN_DIR="$HOME/.local/bin"
        place 0755 "" "$TMP/pulse" "$BIN_DIR"
    fi
else
    place 0755 "" "$TMP/pulse" "$BIN_DIR"
fi

# --- install the setuid helper (best-effort) ---------------------------------
if [ "$WANT_HELPER" -eq 1 ]; then
    if [ -f "$TMP/pulse-helper" ]; then
        chmod +x "$TMP/pulse-helper"
        if place 4755 root "$TMP/pulse-helper" "$HELPER_DIR"; then
            echo "pulse: installed setuid helper at $HELPER_DIR/pulse-helper"
        else
            echo "pulse: could not install the helper (needs root); skipping" >&2
        fi
    else
        echo "pulse: this release has no helper binary; skipping helper" >&2
    fi
fi

# --- record mode + finish ----------------------------------------------------
case "$INSTALL_TYPE" in
    global) record_mode system ;;
    *) record_mode user ;;
esac

echo
echo "Installed $BIN_DIR/pulse"
case ":$PATH:" in
    *":$BIN_DIR:"*) : ;;
    *) echo "Add $BIN_DIR to your PATH:"; echo "    export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac
