#!/usr/bin/env bash
#
# Uninstall Pulse. Removes the pulse binary from wherever it's installed (user
# or system) and, for a system install, the setuid helper. Pass --purge to also
# delete ~/.pulse (config + install database) and Pulse's system lib dir.
#
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
    SYS_LIB="$PREFIX/lib/pulse"
else
    HELPER="$PREFIX/libexec/pulse-helper"
    SYS_LIB="$PREFIX/lib"
fi

# remove <path> [recursive] — with sudo only if the parent isn't writable.
remove() {
    local p="$1" flag="${2:-}"
    [ -e "$p" ] || return 0
    if [ -w "$(dirname "$p")" ]; then
        rm -f $flag "$p"
    elif command -v sudo >/dev/null 2>&1; then
        sudo rm -f $flag "$p"
    else
        echo "pulse: cannot remove $p (need root)" >&2
        return 0
    fi
    echo "removed $p"
}

remove "$USER_BIN"
remove "$SYS_BIN"
remove "$HELPER"

if [ "$PURGE" -eq 1 ]; then
    remove "$SYS_LIB" -r
    if [ -d "$HOME/.pulse" ]; then
        rm -rf "$HOME/.pulse"
        echo "removed $HOME/.pulse"
    fi
fi

echo
echo "Pulse uninstalled."
if [ "$PURGE" -eq 0 ]; then
    echo "(Kept ~/.pulse and anything Pulse installed; run with --purge to remove them.)"
fi
