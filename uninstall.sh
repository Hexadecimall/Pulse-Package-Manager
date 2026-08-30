#!/usr/bin/env bash
#
# Uninstall Pulse. Detects whether Pulse was installed system-wide or per-user
# and removes that one — only the specific `pulse` (and, for a system install,
# `pulse-helper`) files. It never recursively deletes a system directory.
#
# Pass --purge to also delete ~/.pulse (Pulse's own config + install database).
# Uses sudo only when a system path isn't writable.
#
# Usage:  ./uninstall.sh [--purge]
#
set -euo pipefail

PURGE=0
for arg in "$@"; do
    case "$arg" in
        --purge) PURGE=1 ;;
        *) echo "usage: uninstall.sh [--purge]" >&2; exit 1 ;;
    esac
done

# Per-OS system prefix (mirrors the installer / paths.rs).
case "$(uname -s)" in
    Darwin) OS="macos" ;;
    Linux) OS="linux" ;;
    *) OS="other" ;;
esac
if [ -n "${PULSE_PREFIX:-}" ]; then
    PREFIX="$PULSE_PREFIX"
elif [ "$OS" = "linux" ]; then
    PREFIX="/usr"
else
    PREFIX="/opt/pulse"
fi

USER_BIN="$HOME/.local/bin/pulse"
SYS_BIN="$PREFIX/bin/pulse"
if [ "$OS" = "linux" ]; then
    HELPER="$PREFIX/libexec/pulse/pulse-helper"
else
    HELPER="$PREFIX/libexec/pulse-helper"
fi

# Which install is it? Prefer the recorded mode; else infer from what exists.
MODE=""
if [ -f "$HOME/.pulse/config" ]; then
    MODE="$(grep -oE 'install_mode *= *"(system|user)"' "$HOME/.pulse/config" | grep -oE '(system|user)' || true)"
fi
if [ -z "$MODE" ]; then
    if [ -e "$SYS_BIN" ]; then MODE="system"; else MODE="user"; fi
fi

# Remove a single file, with sudo only if its directory isn't writable.
remove_file() {
    local p="$1"
    [ -e "$p" ] || { echo "not found: $p"; return 0; }
    if [ -w "$(dirname "$p")" ]; then
        rm -f "$p"
    elif command -v sudo >/dev/null 2>&1; then
        sudo rm -f "$p"
    else
        echo "pulse: cannot remove $p (need root)" >&2
        return 0
    fi
    echo "removed $p"
}

echo "pulse: uninstalling the $MODE install..."
if [ "$MODE" = "system" ]; then
    remove_file "$SYS_BIN"
    remove_file "$HELPER"
else
    remove_file "$USER_BIN"
fi

if [ "$PURGE" -eq 1 ] && [ -d "$HOME/.pulse" ]; then
    rm -rf "$HOME/.pulse"
    echo "removed $HOME/.pulse"
fi

echo
echo "Pulse uninstalled."
if [ "$PURGE" -eq 0 ]; then
    echo "(Kept ~/.pulse and anything Pulse installed; run with --purge to remove ~/.pulse.)"
fi
