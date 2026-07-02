#!/bin/sh
# Graphenium one-line installer
# curl -fsSL https://raw.githubusercontent.com/lambda-alpha-labs/Graphenium/main/install.sh | sh
set -e

REPO="lambda-alpha-labs/Graphenium"
BINARY="gm"

# Detect platform
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
    x86_64|amd64)  ARCH="x86_64" ;;
    arm64|aarch64) ARCH="arm64"  ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    darwin)  PLATFORM="darwin" ;;
    linux)   PLATFORM="linux" ;;
    *)       echo "Unsupported OS: $OS"; exit 1 ;;
esac

ARTIFACT="$BINARY-$PLATFORM-$ARCH"

# Determine install directory
if [ -w /usr/local/bin ]; then
    INSTALL_DIR=/usr/local/bin
else
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

echo "Installing Graphenium $ARTIFACT to $INSTALL_DIR..."

# Try to get the latest release, fall back to cargo
LATEST_URL="https://github.com/$REPO/releases/latest/download/$ARTIFACT.tar.gz"

if curl -fsSL --head "$LATEST_URL" > /dev/null 2>&1; then
    TMPDIR="$(mktemp -d)"
    curl -fsSL "$LATEST_URL" | tar -xz -C "$TMPDIR"
    install -m 755 "$TMPDIR/$BINARY" "$INSTALL_DIR/$BINARY"
    rm -rf "$TMPDIR"
elif command -v cargo > /dev/null 2>&1; then
    cargo install --locked graphenium
else
    echo "No prebuilt binary found and cargo is not installed."
    echo "Install Rust from https://rustup.rs, then: cargo install --locked graphenium"
    exit 1
fi

echo "Graphenium ($BINARY) installed to $INSTALL_DIR/$BINARY"

# Check PATH
case ":$PATH:" in
    *:"$INSTALL_DIR":*) ;;
    *) echo "Note: add $INSTALL_DIR to your PATH if not already there." ;;
esac

echo ""
echo "Quick start:"
echo "  $BINARY run . --no-semantic --no-viz"
echo "  $BINARY query \"authentication flow\""
echo "  $BINARY doctor"
