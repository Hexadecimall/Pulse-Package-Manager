#!/usr/bin/env bash
#
# Uninstall Pulse. Detects whether Pulse was installed system-wide or per-user
# and removes the `pulse` binary (and, for a system install, `pulse-helper`).
#
# --purge additionally removes every file Pulse recorded installing — read from
# ~/.pulse/db.json, so only the exact files Pulse added (wherever they are,
# including things it placed in /usr/lib) are removed — and then ~/.pulse itself.
#
# Only specific files are ever deleted. This never does `rm -rf` on a shared
# system directory.
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
    [ -n "$p" ] || return 0
    [ -e "$p" ] || return 0
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

if [ "$PURGE" -eq 1 ]; then
    DB="$HOME/.pulse/db.json"
    if [ -f "$DB" ]; then
        echo "pulse: removing the files Pulse installed..."
        # Each recorded install's "path" — remove exactly those files, nothing else.
        grep -oE '"path"[[:space:]]*:[[:space:]]*"[^"]+"' "$DB" \
            | sed -E 's/.*"([^"]+)"$/\1/' \
            | while IFS= read -r f; do
                remove_file "$f"
            done
    fi
    if [ -d "$HOME/.pulse" ]; then
        rm -rf "$HOME/.pulse"   # Pulse's own home directory
        echo "removed $HOME/.pulse"
    fi
fi

echo
echo "Pulse uninstalled."
if [ "$PURGE" -eq 0 ]; then
    echo "(Kept ~/.pulse and the packages Pulse installed; run with --purge to remove them.)"
fi
