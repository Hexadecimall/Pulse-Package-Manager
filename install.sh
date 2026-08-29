#!/usr/bin/env bash
#
# Install Pulse from a prebuilt release — no building required.
#
# Usage:
#   ./install.sh [stable|beta|dev] [--as-user]
#
#   stable (default)  the latest thoroughly-tested release
#   beta              the newest confirmed-but-lightly-tested prerelease
#   dev               the newest experimental build
#   --as-user         install into ~/.pulse/bin (no root); otherwise the binary
#                     is installed to /usr/local/bin setuid-root, so Pulse can
#                     drive system package managers without a password
#
set -euo pipefail

OWNER="Hexadecimall"
REPO="Pulse-Package-Manager"
PREFIX="${PREFIX:-/usr/local/bin}"

CHANNEL="stable"
AS_USER=0
for arg in "$@"; do
    case "$arg" in
        stable|beta|dev) CHANNEL="$arg" ;;
        --as-user) AS_USER=1 ;;
        *) echo "usage: install.sh [stable|beta|dev] [--as-user]" >&2; exit 1 ;;
    esac
done

# A system install needs root; re-run under sudo before doing anything else.
if [ "$AS_USER" -eq 0 ] && [ "$(id -u)" -ne 0 ]; then
    echo "pulse: a system install needs root; re-running with sudo..."
    exec sudo -E bash "$0" "$@"
fi

# Detect the platform, matching the release asset naming.
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

# Resolve the download URL for the requested channel.
case "$CHANNEL" in
    stable) URL="https://github.com/$OWNER/$REPO/releases/latest/download/$ASSET" ;;
    dev) URL="https://github.com/$OWNER/$REPO/releases/download/dev/$ASSET" ;;
    beta)
        TAG="$(curl -fsSL "https://api.github.com/repos/$OWNER/$REPO/releases?per_page=30" \
            | grep -o '"tag_name": *"[^"]*beta[^"]*"' \
            | head -1 \
            | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
        [ -n "$TAG" ] || { echo "no beta release is available yet" >&2; exit 1; }
        URL="https://github.com/$OWNER/$REPO/releases/download/$TAG/$ASSET"
        ;;
esac

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "pulse: downloading $ASSET ($CHANNEL)..."
curl -fSL "$URL" -o "$TMP/$ASSET"
tar -C "$TMP" -xzf "$TMP/$ASSET"
[ -f "$TMP/pulse" ] || { echo "release archive did not contain 'pulse'" >&2; exit 1; }
chmod +x "$TMP/pulse"

if [ "$AS_USER" -eq 1 ]; then
    DEST="$HOME/.pulse/bin/pulse"
    install -d "$HOME/.pulse/bin"
    install -m 0755 "$TMP/pulse" "$DEST"
    echo
    echo "Installed $DEST"
    echo 'Add this to your shell profile so it (and directly-installed binaries) are found:'
    echo '    export PATH="$HOME/.pulse/bin:$PATH"'
else
    DEST="$PREFIX/pulse"
    install -d "$PREFIX"
    # mode 4755: setuid + rwxr-xr-x, owned by root.
    install -o root -m 4755 "$TMP/pulse" "$DEST"
    echo
    echo "Installed $DEST (setuid-root)"
    echo 'Add this to your shell profile so directly-installed binaries are found:'
    echo '    export PATH="$HOME/.pulse/bin:$PATH"'
fi
