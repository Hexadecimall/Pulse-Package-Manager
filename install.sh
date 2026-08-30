#!/usr/bin/env bash
#
# Install Pulse from a prebuilt release — no building required.
#
# Usage:
#   ./install.sh [stable|beta|dev] [--as-user] [--as-root]
#
#   stable (default)  latest thoroughly-tested release
#   beta              newest confirmed-but-lightly-tested prerelease
#   dev               newest experimental build
#   --as-root         system install to /usr/local/bin, setuid-root (default)
#   --as-user         user install to ~/.local/bin, no root
#
# A system install needs root. If root isn't available (no sudo, or you decline),
# the install falls back to a user install in ~/.local/bin.
#
set -euo pipefail

OWNER="Hexadecimall"
REPO="Pulse-Package-Manager"

CHANNEL="stable"
MODE="system"   # default
for arg in "$@"; do
    case "$arg" in
        stable|beta|dev) CHANNEL="$arg" ;;
        --as-user) MODE="user" ;;
        --as-root) MODE="system" ;;
        *) echo "usage: install.sh [stable|beta|dev] [--as-user] [--as-root]" >&2; exit 1 ;;
    esac
done

# A system install needs root. Re-run under sudo if we can; otherwise fall back
# to a user install rather than failing.
if [ "$MODE" = "system" ] && [ "$(id -u)" -ne 0 ]; then
    if command -v sudo >/dev/null 2>&1; then
        echo "pulse: system install needs root; re-running with sudo..."
        exec sudo -E bash "$0" "$@"
    else
        echo "pulse: no root available — falling back to a user install in ~/.local/bin" >&2
        MODE="user"
    fi
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

# Record the install mode so the installed binary defaults to it. Written to the
# invoking user's ~/.pulse/config (not root's, when we escalated via sudo).
record_mode() {
    local home
    home="$(eval echo "~${SUDO_USER:-$USER}")"
    install -d "$home/.pulse"
    printf 'install_mode = "%s"\n' "$1" > "$home/.pulse/config"
    [ -n "${SUDO_USER:-}" ] && chown "$SUDO_USER" "$home/.pulse/config" "$home/.pulse" 2>/dev/null || true
}

if [ "$MODE" = "user" ]; then
    DEST="$HOME/.local/bin/pulse"
    install -d "$HOME/.local/bin"
    install -m 0755 "$TMP/pulse" "$DEST"
    record_mode user
    echo
    echo "Installed $DEST (user mode)"
    echo 'Make sure ~/.local/bin is on your PATH:'
    echo '    export PATH="$HOME/.local/bin:$PATH"'
else
    DEST="/usr/local/bin/pulse"
    install -d /usr/local/bin
    # mode 4755: setuid + rwxr-xr-x, owned by root.
    install -o root -m 4755 "$TMP/pulse" "$DEST"
    record_mode system
    echo
    echo "Installed $DEST (system mode, setuid-root)"
    echo 'Make sure ~/.local/bin is on your PATH for --as-user installs:'
    echo '    export PATH="$HOME/.local/bin:$PATH"'
fi
