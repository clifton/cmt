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
    local test_file="test_${provider// /_}.txt"
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

    # Verify conventional commit format (allows optional scope: type(scope): subject or type: subject)
    if ! echo "$message" | grep -qE "^[a-z]+(\([^)]*\))?: .+$"; then
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

    # Verify the commit message (allows optional scope: type(scope): subject or type: subject)
    if ! echo "$commit_msg" | grep -qE "^[a-z]+(\([^)]*\))?: .+$"; then
        echo "❌ Failed: Commit message doesn't follow conventional format"
        echo "Message was: $commit_msg"
        exit 1
    fi

    echo "✓ $provider test successful"
}

# Function to test invalid model error handling
test_invalid_model() {
    local provider=$1
    local provider_flag=$2
    local invalid_model=$3

    echo -e "\n🧪 Testing invalid model error handling for $provider with model '$invalid_model'..."

    # Create and stage a test file
    local test_file="test_invalid_model_${provider}.txt"
    echo "This is a test file for invalid model error handling" > "$test_file"
    git add "$test_file"

    # Run the command and capture both stdout and stderr
    local output
    if ! output=$("$CMT_BIN" $provider_flag --model "$invalid_model" 2>&1); then
        # Command should fail, so this is expected
        echo "Command failed as expected"
    else
        echo "❌ Failed: Command succeeded but should have failed with invalid model"
        exit 1
    fi

    # Check for invalid model error message
    if ! echo "$output" | grep -q "Invalid model:"; then
        echo "❌ Failed: Missing 'Invalid model:' error message"
        echo "Output was:"
        echo "$output"
        exit 1
    fi

    # Check for the provider name in the error message
    if ! echo "$output" | grep -q -i "$provider"; then
        echo "❌ Failed: Provider name not mentioned in error message"
        echo "Output was:"
        echo "$output"
        exit 1
    fi

    # Check for bulleted list format
    if ! echo "$output" | grep -q "  • "; then
        echo "❌ Failed: Available models not displayed as a bulleted list"
        echo "Output was:"
        echo "$output"
        exit 1
    fi

    # Check that the invalid model is mentioned in the error
    if ! echo "$output" | grep -q "$invalid_model"; then
        echo "❌ Failed: Invalid model name not mentioned in error message"
        echo "Output was:"
        echo "$output"
        exit 1
    fi

    # Check for "Available models:" text
    if ! echo "$output" | grep -q "Available models:"; then
        echo "❌ Failed: Missing 'Available models:' text in error message"
        echo "Output was:"
        echo "$output"
        exit 1
    fi

    echo "✓ Invalid model error handling test successful for $provider"
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

# Test Gemini (default)
test_provider "Gemini" "--provider gemini" ""

# Test Claude
test_provider "Claude" "--provider claude" ""

# Test OpenAI
test_provider "OpenAI" "--provider openai" ""

# Test Gemini with hint
test_provider "Gemini with hint" "--provider gemini" "Fix the login timeout issue"

# Test Claude with hint
test_provider "Claude with hint" "--provider claude" "Update API documentation"

# Test OpenAI with hint
test_provider "OpenAI with hint" "--provider openai" "Refactor for performance"

# Test invalid model error handling
test_invalid_model "Gemini" "--provider gemini" "nonexistent-model-123"
test_invalid_model "Claude" "--provider claude" "nonexistent-model-456"
test_invalid_model "OpenAI" "--provider openai" "nonexistent-model-789"

echo -e "\n✨ All integration tests passed!"

# Clean up after successful run
cd ..
rm -rf test-repo