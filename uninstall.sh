#!/usr/bin/env bash
#
# Remove the Pulse binary. Pass --purge to also delete ~/.pulse (config,
# install database, and any directly-installed binaries).
#
# Usage:  sudo ./uninstall.sh [--purge]
#
set -euo pipefail

PREFIX="${PREFIX:-/usr/local/bin}"
DEST="$PREFIX/pulse"

if [ "$(id -u)" -ne 0 ]; then
    echo "pulse: needs root to remove $DEST; re-running with sudo..."
    exec sudo -E bash "$0" "$@"
fi

if [ -e "$DEST" ]; then
    rm -f "$DEST"
    echo "Removed $DEST"
else
    echo "No binary at $DEST"
fi

if [ "${1:-}" = "--purge" ]; then
    HOME_DIR="$(eval echo "~${SUDO_USER:-}")"
    PULSE_HOME="$HOME_DIR/.pulse"
    if [ -d "$PULSE_HOME" ]; then
        rm -rf "$PULSE_HOME"
        echo "Removed $PULSE_HOME"
    fi
fi
