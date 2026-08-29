#!/usr/bin/env bash
#
# Build Pulse and install it setuid-root.
#
# Installing setuid-root lets `pulse` drive system package managers (apt, dnf,
# pacman) without asking for a password each time — it already has the
# privileges. Homebrew is the exception: it refuses to run as root, so Pulse
# drops back to the invoking user for every `brew` call.
#
# Usage:  sudo ./install.sh
#
set -euo pipefail

PREFIX="${PREFIX:-/usr/local/bin}"
BIN_NAME="pulse"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Re-run ourselves under sudo if we aren't root — the install step needs it.
if [ "$(id -u)" -ne 0 ]; then
    echo "pulse: needs root to install setuid; re-running with sudo..."
    exec sudo -E bash "$0" "$@"
fi

# Build as the invoking user, not as root: cargo's caches and toolchain belong
# to the real user, and building as root litters root-owned files in target/.
BUILD_USER="${SUDO_USER:-root}"
echo "pulse: building release (as ${BUILD_USER})..."
cd "$REPO_DIR"
if [ "$BUILD_USER" != "root" ]; then
    sudo -u "$BUILD_USER" cargo build --release
else
    cargo build --release
fi

SRC="$REPO_DIR/target/release/$BIN_NAME"
DEST="$PREFIX/$BIN_NAME"

echo "pulse: installing to $DEST (root-owned, setuid)..."
install -d "$PREFIX"
# mode 4755: setuid bit + rwxr-xr-x, owned by root.
install -o root -g "$(id -gn root)" -m 4755 "$SRC" "$DEST"

echo
echo "Installed $DEST"
echo "Add this to your shell profile so directly-installed binaries are found:"
echo '    export PATH="$HOME/.pulse/bin:$PATH"'
