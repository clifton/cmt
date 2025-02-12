#!/bin/bash
set -e  # Exit on any error

# Function to test commit message generation with a specific provider
test_provider() {
    local provider=$1
    local provider_flag=$2
    local hint=$3
    local hint_flag=""

    if [ -n "$hint" ]; then
        hint_flag="--hint \"$hint\""
        echo -e "\n🤖 Testing $provider with hint: '$hint'..."
    else
        echo -e "\n🤖 Testing $provider..."
    fi

    # Create and stage a test file
    echo "Test content for $provider" > "test_${provider}.txt"
    git add "test_${provider}.txt"

    # Generate commit message
    if [ -n "$hint" ]; then
        message=$(../target/debug/cmt --message-only $provider_flag --hint "$hint")
    else
        message=$(../target/debug/cmt --message-only $provider_flag)
    fi

    echo "Generated message:"
    echo "$message"

    # Verify conventional commit format
    echo "$message" | grep -q "^[a-z]\+: .*$" || {
        echo "❌ Failed: Message doesn't follow conventional commit format"
        exit 1
    }

    # Make the commit
    git commit -F <(echo "$message")
    commit_msg=$(git log -1 --pretty=%B)
    echo "Commit message:"
    echo "$commit_msg"

    # Verify the commit message
    git log -1 --pretty=%B | grep -q "^[a-z]\+: .*$" || {
        echo "❌ Failed: Commit message doesn't follow conventional format"
        exit 1
    }

    echo "✓ $provider test successful"
}

# Clean up from previous runs
echo "🧹 Cleaning up previous test artifacts..."
rm -rf test-repo
echo "✓ Cleanup complete"

echo "🔍 Setting up test repository..."
mkdir -p test-repo
cd test-repo
git init
echo "✓ Git repository initialized"

echo -e "\n📊 Testing diff display..."
echo "Initial content" > test.txt
git add test.txt
../target/debug/cmt --show-diff
# Verify diff output contains our file
../target/debug/cmt --show-diff | grep -q "test.txt" || {
    echo "❌ Failed: Diff output doesn't show test.txt"
    exit 1
}
echo "✓ Diff display working"

# Test Claude (default)
test_provider "Claude" "" ""

# Test OpenAI
test_provider "OpenAI" "--openai" ""

# Test Claude with hint
test_provider "Claude with hint" "" "Fix the login timeout issue"

# Test OpenAI with hint
test_provider "OpenAI with hint" "--openai" "Update API documentation"

echo -e "\n✨ All integration tests passed!"

# Clean up after successful run
cd ..
rm -rf test-repo