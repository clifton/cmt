#!/bin/bash
set -e  # Exit on any error

# Store the path to the binary
CMT_BIN="$(pwd)/target/debug/cmt"

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

    # Create and stage a test file with meaningful content
    local test_file="test_${provider}.txt"
    echo "Feature: User Authentication
- Add login form component
- Implement password validation
- Add error handling for invalid credentials" > "$test_file"
    git add "$test_file"

    # Generate commit message
    local message
    if [ -n "$hint" ]; then
        message=$("$CMT_BIN" --message-only $provider_flag --hint "$hint")
    else
        message=$("$CMT_BIN" --message-only $provider_flag)
    fi

    echo "Generated message:"
    echo "$message"

    # Verify conventional commit format
    if ! echo "$message" | grep -q "^[a-z]\+: .*$"; then
        echo "❌ Failed: Message doesn't follow conventional commit format"
        echo "Message was: $message"
        exit 1
    fi

    # Make the commit
    git commit -F <(echo "$message")
    local commit_msg
    commit_msg=$(git log -1 --pretty=%B)
    echo "Commit message:"
    echo "$commit_msg"

    # Verify the commit message
    if ! echo "$commit_msg" | grep -q "^[a-z]\+: .*$"; then
        echo "❌ Failed: Commit message doesn't follow conventional format"
        echo "Message was: $commit_msg"
        exit 1
    fi

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
git config --local user.email "test@example.com"
git config --local user.name "Test User"
echo "✓ Git repository initialized"

echo -e "\n📊 Testing diff statistics display..."
echo "# Authentication Service
This component handles user authentication and session management.

## Features
- Login functionality
- Password validation
- Session handling" > test.txt
git add test.txt

# Capture the full output to verify both diff stats and file name
output=$("$CMT_BIN")

# Check for "Diff Statistics:" header
if ! echo "$output" | grep -q "Diff Statistics:"; then
    echo "❌ Failed: Missing 'Diff Statistics:' header in output"
    echo "Output was:"
    echo "$output"
    exit 1
fi

# Check for the test file in the diff stats
if ! echo "$output" | grep -q "test.txt"; then
    echo "❌ Failed: Diff statistics output doesn't show test.txt"
    echo "Output was:"
    echo "$output"
    exit 1
fi

# Check for file change indicators
if ! echo "$output" | grep -q "file.*changed"; then
    echo "❌ Failed: Missing file change statistics"
    echo "Output was:"
    echo "$output"
    exit 1
fi

echo "✓ Diff statistics display working"

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