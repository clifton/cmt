//! Git commit creation that respects hooks.
//!
//! This module shells out to `git commit` rather than using git2 directly,
//! ensuring that all git hooks (pre-commit, commit-msg, etc.) are executed.

use git2::Repository;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

/// Errors that can occur when creating a commit.
#[derive(Debug)]
pub enum CommitError {
    /// The pre-commit hook failed (exit code 1).
    PreCommitFailed,
    /// The commit-msg hook failed.
    CommitMsgFailed,
    /// A general git error occurred.
    GitError(String),
    /// Failed to create or write to the temp file.
    TempFileError(std::io::Error),
    /// Failed to parse the commit output.
    ParseError,
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitError::PreCommitFailed => write!(f, "pre-commit hook failed"),
            CommitError::CommitMsgFailed => write!(f, "commit-msg hook failed"),
            CommitError::GitError(msg) => write!(f, "git error: {}", msg),
            CommitError::TempFileError(e) => write!(f, "temp file error: {}", e),
            CommitError::ParseError => write!(f, "failed to parse commit output"),
        }
    }
}

impl std::error::Error for CommitError {}

/// Options for creating a commit.
#[derive(Debug, Default)]
pub struct CommitOptions {
    /// Skip pre-commit and commit-msg hooks.
    pub no_verify: bool,
}

/// Result of a successful commit.
#[derive(Debug)]
pub struct CommitResult {
    /// The commit object ID (SHA).
    pub oid: String,
}

/// Create a commit with the given message, respecting git hooks.
///
/// This function shells out to `git commit -F <tempfile>` to ensure all
/// git hooks are executed (pre-commit, commit-msg, etc.).
pub fn create_commit(
    repo: &Repository,
    message: &str,
    options: &CommitOptions,
) -> Result<CommitResult, CommitError> {
    // Write message to a temp file
    let mut temp_file = NamedTempFile::new().map_err(CommitError::TempFileError)?;
    temp_file
        .write_all(message.as_bytes())
        .map_err(CommitError::TempFileError)?;
    temp_file.flush().map_err(CommitError::TempFileError)?;

    // Get the repo's working directory
    let workdir = repo
        .workdir()
        .ok_or_else(|| CommitError::GitError("bare repository".to_string()))?;

    // Build the git commit command
    let mut cmd = Command::new("git");
    cmd.current_dir(workdir);
    cmd.arg("commit");
    cmd.arg("-F");
    cmd.arg(temp_file.path());

    if options.no_verify {
        cmd.arg("--no-verify");
    }

    // Run the command
    let output = cmd
        .output()
        .map_err(|e| CommitError::GitError(format!("failed to execute git: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}{}", stdout, stderr);

        // Check for hook failures
        // Git typically exits with code 1 for hook failures
        if let Some(code) = output.status.code() {
            if code == 1 {
                // Try to determine which hook failed from the output
                let lower = combined.to_lowercase();
                if lower.contains("pre-commit") {
                    return Err(CommitError::PreCommitFailed);
                }
                if lower.contains("commit-msg") {
                    return Err(CommitError::CommitMsgFailed);
                }
                // If no specific hook mentioned but exit code 1, likely pre-commit
                // since it runs first
                if !lower.contains("nothing to commit") && !lower.contains("no changes") {
                    return Err(CommitError::PreCommitFailed);
                }
            }
        }

        return Err(CommitError::GitError(combined.trim().to_string()));
    }

    // Parse the commit hash from output
    // Format: "[branch hash] message" or "[branch (root-commit) hash] message"
    let stdout = String::from_utf8_lossy(&output.stdout);
    let oid = parse_commit_hash(&stdout).ok_or(CommitError::ParseError)?;

    Ok(CommitResult { oid })
}

/// Parse the commit hash from git commit output.
///
/// Git outputs something like:
/// - `[main abc1234] commit message`
/// - `[main (root-commit) abc1234] commit message`
fn parse_commit_hash(output: &str) -> Option<String> {
    // Find the first line that looks like a commit output
    for line in output.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // Find the closing bracket
            if let Some(bracket_end) = line.find(']') {
                let inside = &line[1..bracket_end];
                // The hash is the last space-separated token before the bracket
                if let Some(hash) = inside.split_whitespace().last() {
                    // Verify it looks like a hash (alphanumeric)
                    if hash.chars().all(|c| c.is_ascii_hexdigit()) && hash.len() >= 7 {
                        return Some(hash.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commit_hash_normal() {
        let output = "[main abc1234] fix: some commit message\n";
        assert_eq!(parse_commit_hash(output), Some("abc1234".to_string()));
    }

    #[test]
    fn test_parse_commit_hash_root_commit() {
        let output = "[main (root-commit) def5678] initial commit\n";
        assert_eq!(parse_commit_hash(output), Some("def5678".to_string()));
    }

    #[test]
    fn test_parse_commit_hash_with_prefix() {
        let output = "Some warning\n[feature/test 1234567] feat: add feature\nSome other output\n";
        assert_eq!(parse_commit_hash(output), Some("1234567".to_string()));
    }

    #[test]
    fn test_parse_commit_hash_no_match() {
        let output = "error: something went wrong\n";
        assert_eq!(parse_commit_hash(output), None);
    }

    #[test]
    fn test_parse_commit_hash_full_sha() {
        let output = "[main abcdef1234567890abcdef1234567890abcdef12] long hash\n";
        assert_eq!(
            parse_commit_hash(output),
            Some("abcdef1234567890abcdef1234567890abcdef12".to_string())
        );
    }
}
