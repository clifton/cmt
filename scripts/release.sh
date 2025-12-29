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

# Check if we're on the main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    error "Must be on 'main' branch to create a release. Current branch: $CURRENT_BRANCH"
fi

# Check for uncommitted changes
if ! git diff-index --quiet HEAD -- || ! git diff --staged --quiet; then
    error "Working directory is not clean. Please commit or stash your changes first."
fi

# Check for required tools
if ! command -v gh >/dev/null 2>&1; then
    warn "GitHub CLI (gh) not found. GitHub release creation will be skipped."
    warn "Install it with: brew install gh"
    HAS_GH_CLI=false
else
    # Check if gh is authenticated
    if ! gh auth status >/dev/null 2>&1; then
        warn "GitHub CLI not authenticated. Run 'gh auth login' first."
        HAS_GH_CLI=false
    else
        HAS_GH_CLI=true
    fi
fi

# Default to patch if no argument provided
BUMP_TYPE=${1:-patch}

# Validate bump type
case "$BUMP_TYPE" in
    major|minor|patch) ;;
    *)
        error "Invalid bump type '$BUMP_TYPE'. Must be one of: major, minor, patch"
        ;;
esac

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)
if [ -z "$CURRENT_VERSION" ]; then
    error "Could not find version in Cargo.toml"
fi

# Pull latest changes from remote
echo "Pulling latest changes from remote..."
if ! git pull; then
    error "Failed to pull latest changes"
fi
info "Repository is up to date"

# Split version into major, minor, and patch numbers
MAJOR=$(echo "$CURRENT_VERSION" | cut -d. -f1)
MINOR=$(echo "$CURRENT_VERSION" | cut -d. -f2)
PATCH=$(echo "$CURRENT_VERSION" | cut -d. -f3)

# Bump version according to type
case "$BUMP_TYPE" in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
esac

NEW_VERSION="$MAJOR.$MINOR.$PATCH"
echo ""
echo "Version bump: ${YELLOW}$CURRENT_VERSION${NC} → ${GREEN}$NEW_VERSION${NC} ($BUMP_TYPE)"
echo ""

# Update version in Cargo.toml
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
rm Cargo.toml.bak
info "Updated Cargo.toml"

# Update Cargo.lock
cargo generate-lockfile >/dev/null 2>&1
info "Updated Cargo.lock"

# Find the previous release tag
PREVIOUS_TAG=$(git describe --tags --abbrev=0 --match "v*" 2>/dev/null || echo "")
if [ -z "$PREVIOUS_TAG" ]; then
    warn "No previous release tag found. This appears to be the first release."
    CHANGELOG_RANGE="HEAD"
else
    info "Previous release: $PREVIOUS_TAG"
    CHANGELOG_RANGE="$PREVIOUS_TAG..HEAD"
fi

# Generate changelog from git commits
generate_changelog() {
    if [ -z "$PREVIOUS_TAG" ]; then
        # For first release, get all commits
        git log --pretty=format:"- %s (%h)" --no-merges | grep -v -E "^- (bump|Bump) version"
    else
        # Get commits since last release, excluding version bump commits
        git log --pretty=format:"- %s (%h)" --no-merges "$CHANGELOG_RANGE" | grep -v -E "^- (bump|Bump) version" || true
    fi
}

CHANGELOG=$(generate_changelog)
if [ -z "$CHANGELOG" ]; then
    CHANGELOG="- No notable changes"
fi

# Create release notes
RELEASE_NOTES=$(cat <<EOF
## What's Changed

$CHANGELOG

**Full Changelog**: https://github.com/clifton/cmt/compare/$PREVIOUS_TAG...v$NEW_VERSION
EOF
)

# Show changelog preview
echo ""
echo "📋 Release Notes Preview:"
echo "─────────────────────────"
echo "$RELEASE_NOTES"
echo "─────────────────────────"
echo ""

# Create git commit and tag
git add Cargo.toml Cargo.lock
git commit -m "bump version to $NEW_VERSION"
git tag -a "v$NEW_VERSION" -m "Version $NEW_VERSION"

info "Created commit and tag v$NEW_VERSION"

# Ask for confirmation before pushing to git
echo ""
read -p "Would you like to push the changes and tag to git? (y/N) " should_push
if [ "$should_push" = "y" ] || [ "$should_push" = "Y" ]; then
    git push && git push origin "v$NEW_VERSION"
    info "Pushed changes and tag to git"

    # Create GitHub release with changelog
    if [ "$HAS_GH_CLI" = "true" ]; then
        echo ""
        read -p "Would you like to create a GitHub release? (y/N) " should_release
        if [ "$should_release" = "y" ] || [ "$should_release" = "Y" ]; then
            echo "Creating GitHub release..."
            echo "$RELEASE_NOTES" | gh release create "v$NEW_VERSION" \
                --title "v$NEW_VERSION" \
                --notes-file - \
                --target main
            info "Created GitHub release v$NEW_VERSION"
        else
            echo "Skipped GitHub release creation"
        fi
    fi
else
    echo "Skipped pushing to git"
    echo ""
    echo "To push manually later, run:"
    echo "  git push && git push origin v$NEW_VERSION"
fi

# Ask for confirmation before publishing to crates.io
echo ""
read -p "Would you like to publish to crates.io? (y/N) " should_publish
if [ "$should_publish" = "y" ] || [ "$should_publish" = "Y" ]; then
    # Verify what will be packaged
    echo ""
    echo "Verifying files to be packaged..."
    echo "Files that will be included:"
    cargo package --list --allow-dirty 2>/dev/null | head -20
    echo "... (showing first 20 files)"
    echo ""

    read -p "Continue with publish? (y/N) " continue_publish
    if [ "$continue_publish" != "y" ] && [ "$continue_publish" != "Y" ]; then
        echo "Aborted publishing"
        exit 0
    fi

    cargo publish
    info "Published cmt v$NEW_VERSION to crates.io"
else
    echo "Skipped publishing to crates.io"
fi

echo ""
info "Release process complete!"
echo ""
echo "Summary:"
echo "  • Version: $NEW_VERSION"
echo "  • Tag: v$NEW_VERSION"
if [ "$should_push" = "y" ] || [ "$should_push" = "Y" ]; then
    echo "  • Pushed to git: Yes"
    if [ "$HAS_GH_CLI" = "true" ] && { [ "$should_release" = "y" ] || [ "$should_release" = "Y" ]; }; then
        echo "  • GitHub release: Yes"
    fi
fi
if [ "$should_publish" = "y" ] || [ "$should_publish" = "Y" ]; then
    echo "  • Published to crates.io: Yes"
fi
