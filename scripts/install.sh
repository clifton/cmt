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

# Get latest version
CURL_HEADERS="-H 'Accept: application/vnd.github+json' -H 'X-GitHub-Api-Version: 2022-11-28'"

if [ -n "$GITHUB_TOKEN" ]; then
    CURL_HEADERS="$CURL_HEADERS -H 'Authorization: Bearer $GITHUB_TOKEN'"
fi

GITHUB_API_RESPONSE=$(eval curl -sL $CURL_HEADERS https://api.github.com/repos/clifton/cmt/releases/latest)
if [ -z "$GITHUB_API_RESPONSE" ]; then
    echo "Error: Empty response from GitHub API"
    exit 1
fi

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
DOWNLOAD_URL="https://github.com/clifton/cmt/releases/download/${LATEST_VERSION}/${BINARY}"
eval curl -sL $CURL_HEADERS "$DOWNLOAD_URL" -o "$TMP_DIR/$BINARY"

# Make binary executable
chmod +x "$TMP_DIR/$BINARY"

# Install binary
INSTALL_DIR="/usr/local/bin"
if [ ! -w "$INSTALL_DIR" ]; then
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