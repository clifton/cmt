#!/bin/sh
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
info() { echo "${GREEN}✓${NC} $1"; }
warn() { echo "${YELLOW}⚠${NC} $1"; }
error() { echo "${RED}✗${NC} $1" >&2; exit 1; }

# Configuration
GITHUB_USER="clifton"
REPO_NAME="cmt"
TAP_REPO="homebrew-tap"
FORMULA_NAME="cmt.rb"

# Get version from argument or Cargo.toml
VERSION=${1:-$(grep '^version = ' Cargo.toml | cut -d'"' -f2)}
if [ -z "$VERSION" ]; then
    error "Could not determine version"
fi

info "Updating Homebrew formula for v$VERSION"

# Check for GitHub CLI
if ! command -v gh >/dev/null 2>&1; then
    error "GitHub CLI (gh) is required. Install with: brew install gh"
fi

# Check if gh is authenticated
if ! gh auth status >/dev/null 2>&1; then
    error "GitHub CLI not authenticated. Run 'gh auth login' first."
fi

# Wait for release assets to be available (GitHub Actions might still be building)
echo "Checking if release assets are available..."
MAX_RETRIES=30
RETRY_DELAY=10
ASSETS_READY=false

for i in $(seq 1 $MAX_RETRIES); do
    ASSET_COUNT=$(gh release view "v$VERSION" --repo "$GITHUB_USER/$REPO_NAME" --json assets --jq '.assets | length' 2>/dev/null || echo "0")
    if [ "$ASSET_COUNT" -ge 4 ]; then
        ASSETS_READY=true
        break
    fi
    if [ "$i" -lt "$MAX_RETRIES" ]; then
        echo "  Waiting for release assets... ($i/$MAX_RETRIES, found $ASSET_COUNT assets)"
        sleep $RETRY_DELAY
    fi
done

if [ "$ASSETS_READY" != "true" ]; then
    error "Release assets not available after $(($MAX_RETRIES * $RETRY_DELAY)) seconds. Make sure binaries are uploaded to the release."
fi

info "Release assets are available"

# Create temp directory
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# Download binaries and compute SHA256
echo "Downloading binaries and computing SHA256 checksums..."

compute_sha256() {
    local binary_name=$1
    local url="https://github.com/$GITHUB_USER/$REPO_NAME/releases/download/v$VERSION/$binary_name"

    if curl -sLf "$url" -o "$TMP_DIR/$binary_name" 2>/dev/null; then
        shasum -a 256 "$TMP_DIR/$binary_name" | cut -d' ' -f1
    else
        echo ""
    fi
}

SHA_DARWIN_ARM64=$(compute_sha256 "cmt-darwin-arm64")
SHA_DARWIN_AMD64=$(compute_sha256 "cmt-darwin-amd64")
SHA_LINUX_ARM64=$(compute_sha256 "cmt-linux-arm64")
SHA_LINUX_AMD64=$(compute_sha256 "cmt-linux-amd64")

# Verify we got all checksums
if [ -z "$SHA_DARWIN_ARM64" ] || [ -z "$SHA_DARWIN_AMD64" ]; then
    error "Failed to download macOS binaries. Make sure they exist in the release."
fi

info "SHA256 (darwin-arm64): $SHA_DARWIN_ARM64"
info "SHA256 (darwin-amd64): $SHA_DARWIN_AMD64"
[ -n "$SHA_LINUX_ARM64" ] && info "SHA256 (linux-arm64):  $SHA_LINUX_ARM64"
[ -n "$SHA_LINUX_AMD64" ] && info "SHA256 (linux-amd64):  $SHA_LINUX_AMD64"

# Clone the tap repository
echo "Cloning tap repository..."
TAP_DIR="$TMP_DIR/$TAP_REPO"
gh repo clone "$GITHUB_USER/$TAP_REPO" "$TAP_DIR" 2>/dev/null || {
    warn "Tap repository doesn't exist. Creating it..."
    gh repo create "$TAP_REPO" --public --description "Homebrew tap for $REPO_NAME" --clone --source="$TMP_DIR"
    mkdir -p "$TAP_DIR"
    cd "$TAP_DIR"
    git init
    gh repo create "$GITHUB_USER/$TAP_REPO" --public --description "Homebrew tap for $REPO_NAME" --source=. --push
}

cd "$TAP_DIR"

# Ensure Formula directory exists
mkdir -p Formula

# Generate the formula
cat > "Formula/$FORMULA_NAME" << EOF
# typed: false
# frozen_string_literal: true

# Homebrew formula for cmt - AI-Powered Git Commit Message Generator
# To install: brew tap $GITHUB_USER/tap && brew install cmt
class Cmt < Formula
  desc "CLI tool that generates commit messages using AI"
  homepage "https://github.com/$GITHUB_USER/$REPO_NAME"
  version "$VERSION"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/$GITHUB_USER/$REPO_NAME/releases/download/v#{version}/cmt-darwin-arm64"
      sha256 "$SHA_DARWIN_ARM64"
    end
    on_intel do
      url "https://github.com/$GITHUB_USER/$REPO_NAME/releases/download/v#{version}/cmt-darwin-amd64"
      sha256 "$SHA_DARWIN_AMD64"
    end
  end
EOF

# Add Linux support only if binaries exist
if [ -n "$SHA_LINUX_ARM64" ] && [ -n "$SHA_LINUX_AMD64" ]; then
    cat >> "Formula/$FORMULA_NAME" << EOF

  on_linux do
    on_arm do
      url "https://github.com/$GITHUB_USER/$REPO_NAME/releases/download/v#{version}/cmt-linux-arm64"
      sha256 "$SHA_LINUX_ARM64"
    end
    on_intel do
      url "https://github.com/$GITHUB_USER/$REPO_NAME/releases/download/v#{version}/cmt-linux-amd64"
      sha256 "$SHA_LINUX_AMD64"
    end
  end
EOF
fi

# Add install and test blocks
cat >> "Formula/$FORMULA_NAME" << 'EOF'

  def install
    binary_name = "cmt-#{OS.kernel_name.downcase}-#{Hardware::CPU.arch == :arm64 ? "arm64" : "amd64"}"
    bin.install binary_name => "cmt"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/cmt --version")
  end
end
EOF

info "Generated formula"

# Create README if it doesn't exist
if [ ! -f "README.md" ]; then
    cat > "README.md" << EOF
# Homebrew Tap for cmt

This is the official Homebrew tap for [cmt](https://github.com/$GITHUB_USER/$REPO_NAME) - an AI-powered git commit message generator.

## Installation

\`\`\`bash
brew tap $GITHUB_USER/tap
brew install cmt
\`\`\`

## Updating

\`\`\`bash
brew update
brew upgrade cmt
\`\`\`

## Uninstalling

\`\`\`bash
brew uninstall cmt
brew untap $GITHUB_USER/tap
\`\`\`
EOF
    info "Created README.md"
fi

# Commit and push
git add -A
if git diff --staged --quiet; then
    info "No changes to commit"
else
    git commit -m "Update cmt to v$VERSION"
    git push origin main || git push origin master
    info "Pushed formula update to tap repository"
fi

echo ""
info "Homebrew formula updated successfully!"
echo ""
echo "Users can now install with:"
echo "  brew tap $GITHUB_USER/tap"
echo "  brew install cmt"
echo ""
echo "Or in one command:"
echo "  brew install $GITHUB_USER/tap/cmt"

