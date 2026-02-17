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
    PreCommitFailed { output: String },
    /// The commit-msg hook failed.
    CommitMsgFailed { output: String },
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
            CommitError::PreCommitFailed { .. } => write!(f, "pre-commit hook failed"),
            CommitError::CommitMsgFailed { .. } => write!(f, "commit-msg hook failed"),
            CommitError::GitError(msg) => write!(f, "git error: {}", msg),
            CommitError::TempFileError(e) => write!(f, "temp file error: {}", e),
            CommitError::ParseError => write!(f, "failed to parse commit output"),
        }
    }
}

impl std::error::Error for CommitError {}

impl CommitError {
    /// Return captured hook output when available.
    pub fn hook_output(&self) -> Option<&str> {
        match self {
            CommitError::PreCommitFailed { output } | CommitError::CommitMsgFailed { output } => {
                let trimmed = output.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }
            _ => None,
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookFailureKind {
    PreCommit,
    CommitMsg,
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
        let combined_trimmed = combined.trim().to_string();

        // Check for hook failures.
        if let Some(failure_kind) = detect_hook_failure(&combined, output.status.code()) {
            return match failure_kind {
                HookFailureKind::PreCommit => Err(CommitError::PreCommitFailed {
                    output: combined_trimmed,
                }),
                HookFailureKind::CommitMsg => Err(CommitError::CommitMsgFailed {
                    output: combined_trimmed,
                }),
            };
        }

        return Err(CommitError::GitError(combined_trimmed));
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

fn detect_hook_failure(output: &str, exit_code: Option<i32>) -> Option<HookFailureKind> {
    // Git typically exits with code 1 for hook failures.
    if exit_code != Some(1) {
        return None;
    }

    let lower = output.to_lowercase();
    if lower.contains("pre-commit") {
        return Some(HookFailureKind::PreCommit);
    }
    if lower.contains("commit-msg") {
        return Some(HookFailureKind::CommitMsg);
    }

    // If no specific hook is mentioned but exit code is 1, it's likely pre-commit.
    if !lower.contains("nothing to commit") && !lower.contains("no changes") {
        return Some(HookFailureKind::PreCommit);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::path::Path;
    #[cfg(unix)]
    use tempfile::tempdir;

    #[cfg(unix)]
    fn run_git(repo_path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .expect("failed to run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

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

    #[test]
    fn test_detect_hook_failure_pre_commit_by_name() {
        let output = "pre-commit hook failed";
        assert_eq!(
            detect_hook_failure(output, Some(1)),
            Some(HookFailureKind::PreCommit)
        );
    }

    #[test]
    fn test_detect_hook_failure_commit_msg_by_name() {
        let output = "commit-msg hook failed";
        assert_eq!(
            detect_hook_failure(output, Some(1)),
            Some(HookFailureKind::CommitMsg)
        );
    }

    #[test]
    fn test_detect_hook_failure_pre_commit_fallback() {
        let output = "lint failed";
        assert_eq!(
            detect_hook_failure(output, Some(1)),
            Some(HookFailureKind::PreCommit)
        );
    }

    #[test]
    fn test_detect_hook_failure_not_hook_failure_nothing_to_commit() {
        let output = "nothing to commit, working tree clean";
        assert_eq!(detect_hook_failure(output, Some(1)), None);
    }

    #[test]
    fn test_detect_hook_failure_non_hook_exit_code() {
        let output = "pre-commit hook failed";
        assert_eq!(detect_hook_failure(output, Some(128)), None);
    }

    #[test]
    fn test_hook_output_accessor() {
        let err = CommitError::PreCommitFailed {
            output: "  lint failed  ".to_string(),
        };
        assert_eq!(err.hook_output(), Some("lint failed"));
    }

    #[test]
    fn test_hook_output_accessor_empty() {
        let err = CommitError::CommitMsgFailed {
            output: "   ".to_string(),
        };
        assert_eq!(err.hook_output(), None);
    }

    #[cfg(unix)]
    #[test]
    fn test_create_commit_returns_pre_commit_output() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let repo_path = temp_dir.path();

        run_git(repo_path, &["init"]);
        run_git(repo_path, &["config", "user.name", "Test User"]);
        run_git(repo_path, &["config", "user.email", "test@example.com"]);

        fs::write(repo_path.join("file.txt"), "content\n").expect("failed to write staged file");
        run_git(repo_path, &["add", "file.txt"]);

        let hook_path = repo_path.join(".git/hooks/pre-commit");
        fs::write(
            &hook_path,
            "#!/bin/sh\necho 'lint failed: trailing whitespace' >&2\nexit 1\n",
        )
        .expect("failed to write pre-commit hook");
        let mut perms = fs::metadata(&hook_path)
            .expect("failed to stat pre-commit hook")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms).expect("failed to chmod pre-commit hook");

        let repo = Repository::open(repo_path).expect("failed to open test repo");
        let error = create_commit(&repo, "test commit", &CommitOptions::default())
            .expect_err("expected failure");

        match error {
            CommitError::PreCommitFailed { output } => {
                assert!(output.contains("lint failed: trailing whitespace"));
            }
            other => panic!("expected pre-commit failure, got {other:?}"),
        }
    }
}
