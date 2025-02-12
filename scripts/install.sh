#!/bin/sh
set -e

# Detect platform
PLATFORM="unknown"
ARCH="unknown"

# Detect OS
case "$(uname -s)" in
    "Darwin")
        PLATFORM="darwin"
        ;;
    "Linux")
        PLATFORM="linux"
        ;;
    *"_NT"* | "MINGW"* | "MSYS"* | "CYGWIN"*)
        PLATFORM="windows"
        ;;
esac

# Detect architecture
case "$(uname -m)" in
    "x86_64" | "amd64")
        ARCH="amd64"
        ;;
    "aarch64" | "arm64")
        ARCH="arm64"
        ;;
esac

if [ "$PLATFORM" = "unknown" ] || [ "$ARCH" = "unknown" ]; then
    echo "Unsupported platform: $(uname -s) $(uname -m)"
    exit 1
fi

# Construct binary name
if [ "$PLATFORM" = "windows" ]; then
    BINARY="cmt-${PLATFORM}-${ARCH}.exe"
else
    BINARY="cmt-${PLATFORM}-${ARCH}"
fi

# Get latest version with debug output
echo "Fetching latest release information..."
AUTH_HEADER=""
if [ -n "$GITHUB_TOKEN" ]; then
    AUTH_HEADER="Authorization: token $GITHUB_TOKEN"
fi

GITHUB_API_RESPONSE=$(curl -sL ${AUTH_HEADER:+-H "$AUTH_HEADER"} https://api.github.com/repos/cliftonk/cmt/releases/latest)
if [ -z "$GITHUB_API_RESPONSE" ]; then
    echo "Error: Empty response from GitHub API"
    exit 1
fi

echo "API Response: $GITHUB_API_RESPONSE"
LATEST_VERSION=$(echo "$GITHUB_API_RESPONSE" | grep '"tag_name":' | cut -d'"' -f4)

if [ -z "$LATEST_VERSION" ]; then
    echo "Error: Could not parse version from GitHub API response"
    exit 1
fi

echo "Installing cmt ${LATEST_VERSION} for ${PLATFORM} ${ARCH}"

# Create temporary directory
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# Download binary
DOWNLOAD_URL="https://github.com/cliftonk/cmt/releases/download/${LATEST_VERSION}/${BINARY}"
echo "Downloading from: ${DOWNLOAD_URL}"
curl -sL ${AUTH_HEADER:+-H "$AUTH_HEADER"} "$DOWNLOAD_URL" -o "$TMP_DIR/$BINARY"

# Make binary executable
chmod +x "$TMP_DIR/$BINARY"

# Install binary
INSTALL_DIR="/usr/local/bin"
if [ ! -w "$INSTALL_DIR" ]; then
    # Try installing to ~/.local/bin if /usr/local/bin is not writable
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

mv "$TMP_DIR/$BINARY" "$INSTALL_DIR/cmt"
echo "Successfully installed cmt to $INSTALL_DIR/cmt"

# Verify installation
if command -v cmt >/dev/null 2>&1; then
    echo "Installation verified. Run 'cmt --help' to get started."
else
    echo "Installation complete, but $INSTALL_DIR is not in your PATH."
    echo "Add $INSTALL_DIR to your PATH or run: export PATH=\"\$PATH:$INSTALL_DIR\""
fi