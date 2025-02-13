#!/bin/sh
set -e

# Check for uncommitted changes
if ! git diff-index --quiet HEAD -- || ! git diff --staged --quiet; then
    echo "Error: Working directory is not clean. Please commit or stash your changes first."
    exit 1
fi

# Default to patch if no argument provided
BUMP_TYPE=${1:-patch}

# Validate bump type
case "$BUMP_TYPE" in
    major|minor|patch) ;;
    *)
        echo "Error: Invalid bump type '$BUMP_TYPE'. Must be one of: major, minor, patch"
        exit 1
        ;;
esac

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)
if [ -z "$CURRENT_VERSION" ]; then
    echo "Error: Could not find version in Cargo.toml"
    exit 1
fi

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
echo "Bumping version from $CURRENT_VERSION to $NEW_VERSION"

# Update version in Cargo.toml
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
rm Cargo.toml.bak

# Update Cargo.lock
cargo generate-lockfile

# Create git commit and tag
git add Cargo.toml Cargo.lock
git commit -m "bump version to $NEW_VERSION"
git tag -a "v$NEW_VERSION" -m "Version $NEW_VERSION"

echo "Successfully bumped version to $NEW_VERSION"

# Ask for confirmation before pushing to git
read -p "Would you like to push the changes and tags to git? (y/N) " should_push
if [ "$should_push" = "y" ] || [ "$should_push" = "Y" ]; then
    git push && git push --tags
    echo "Successfully pushed changes to git"
else
    echo "Skipped pushing to git"
fi

# Ask for confirmation before publishing to crates.io
read -p "Would you like to publish to crates.io? (y/N) " should_publish
if [ "$should_publish" = "y" ] || [ "$should_publish" = "Y" ]; then
    cargo publish
    echo "Successfully published to crates.io"
else
    echo "Skipped publishing to crates.io"
fi