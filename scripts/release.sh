#!/bin/sh
set -e

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
echo "Run 'git push && git push --tags' to publish the release"