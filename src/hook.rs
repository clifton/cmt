//! `prepare-commit-msg` git hook integration.
//!
//! Installing the hook lets cmt fill in the commit message inside plain
//! `git commit` (and IDE / `git gui` / lazygit commit boxes) — not just when the
//! user types `cmt`. The installed hook calls `cmt prepare-commit-msg "$1" "$2"`;
//! cmt writes the generated message into the commit-message file and always
//! exits 0 so it can never block a commit.

use std::fs;
use std::path::{Path, PathBuf};

use git2::Repository;

const BEGIN_MARKER: &str = "### BEGIN CMT HOOK ###";
const END_MARKER: &str = "### END CMT HOOK ###";
const HOOK_NAME: &str = "prepare-commit-msg";

/// Hook error type.
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("could not determine the current executable path: {0}")]
    Exe(String),
}

/// Resolve the git hooks directory, honoring `core.hooksPath` (used by Husky
/// and friends) and falling back to `<gitdir>/hooks`.
pub fn hooks_dir(repo: &Repository) -> PathBuf {
    if let Ok(cfg) = repo.config() {
        if let Ok(custom) = cfg.get_string("core.hooksPath") {
            if !custom.trim().is_empty() {
                let p = PathBuf::from(custom);
                return if p.is_absolute() {
                    p
                } else {
                    // Relative to the working tree, per git semantics.
                    repo.workdir().unwrap_or_else(|| repo.path()).join(p)
                };
            }
        }
    }
    repo.path().join("hooks")
}

/// Decide whether the hook should generate a message for this commit.
///
/// Mirrors the conservative rule used by gptcommit/aicommits: only generate for
/// a fresh, source-less commit. Skip when the user supplied `-m`/`-F` (message),
/// a commit template, or a merge/squash/amend message — clobbering those is the
/// fastest way to get the hook uninstalled.
pub fn hook_should_generate(commit_source: Option<&str>) -> bool {
    match commit_source.map(str::trim) {
        None | Some("") => true,
        // "message" (-m/-F), "template" (-t), "merge", "squash", "commit" (amend)
        Some(_) => false,
    }
}

/// The hook script body that invokes cmt with the given absolute exe path.
fn hook_block(exe: &str) -> String {
    format!(
        "{BEGIN_MARKER}\n\"{exe}\" prepare-commit-msg \"$1\" \"$2\" \"$3\" || true\n{END_MARKER}\n"
    )
}

/// Install (or update) the cmt `prepare-commit-msg` hook. Coexists with any
/// existing hook content by appending a marker-delimited block. Returns the
/// hook file path.
pub fn install(repo: &Repository) -> Result<PathBuf, HookError> {
    let exe = std::env::current_exe()
        .map_err(|e| HookError::Exe(e.to_string()))?
        .to_string_lossy()
        .into_owned();

    let dir = hooks_dir(repo);
    fs::create_dir_all(&dir)?;
    let path = dir.join(HOOK_NAME);

    let existing = fs::read_to_string(&path).unwrap_or_default();
    let cleaned = strip_block(&existing);

    let new_content = if cleaned.trim().is_empty() {
        format!("#!/bin/sh\n{}", hook_block(&exe))
    } else {
        // Preserve the user's existing hook, append our block.
        let mut c = cleaned;
        if !c.ends_with('\n') {
            c.push('\n');
        }
        c.push_str(&hook_block(&exe));
        c
    };

    fs::write(&path, new_content)?;
    make_executable(&path)?;
    Ok(path)
}

/// Remove the cmt block from the hook. If nothing else remains (just a shebang),
/// the hook file is removed entirely. Returns whether a hook was present.
pub fn uninstall(repo: &Repository) -> Result<bool, HookError> {
    let path = hooks_dir(repo).join(HOOK_NAME);
    let Ok(existing) = fs::read_to_string(&path) else {
        return Ok(false);
    };
    if !existing.contains(BEGIN_MARKER) {
        return Ok(false);
    }

    let cleaned = strip_block(&existing);
    let meaningful = cleaned
        .lines()
        .any(|l| !l.trim().is_empty() && !l.trim_start().starts_with("#!"));

    if meaningful {
        fs::write(&path, cleaned)?;
    } else {
        fs::remove_file(&path)?;
    }
    Ok(true)
}

/// Remove the marker-delimited cmt block (and any all-comment script) from hook
/// text, leaving the rest intact.
fn strip_block(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut in_block = false;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed == BEGIN_MARKER {
            in_block = true;
            continue;
        }
        if trimmed == END_MARKER {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
        }
    }
    out
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), HookError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), HookError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_matrix() {
        assert!(hook_should_generate(None));
        assert!(hook_should_generate(Some("")));
        for src in ["message", "template", "merge", "squash", "commit"] {
            assert!(!hook_should_generate(Some(src)), "should skip {src}");
        }
    }

    #[test]
    fn test_strip_block_removes_only_cmt_block() {
        let content = format!(
            "#!/bin/sh\necho keep-me\n{BEGIN_MARKER}\n\"cmt\" prepare-commit-msg \"$1\" || true\n{END_MARKER}\necho also-keep\n"
        );
        let stripped = strip_block(&content);
        assert!(stripped.contains("echo keep-me"));
        assert!(stripped.contains("echo also-keep"));
        assert!(!stripped.contains("prepare-commit-msg"));
        assert!(!stripped.contains(BEGIN_MARKER));
    }

    #[test]
    fn test_hook_block_is_marker_wrapped_and_calls_cmt() {
        let b = hook_block("/usr/local/bin/cmt");
        assert!(b.starts_with(BEGIN_MARKER));
        assert!(b.trim_end().ends_with(END_MARKER));
        assert!(b.contains("/usr/local/bin/cmt"));
        assert!(b.contains("prepare-commit-msg"));
        assert!(b.contains("|| true"), "must never fail the commit");
    }

    #[test]
    fn test_install_and_uninstall_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(tmp.path()).unwrap();

        let path = install(&repo).unwrap();
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains(BEGIN_MARKER));
        assert!(content.contains("prepare-commit-msg"));

        // Idempotent: installing again doesn't duplicate the block.
        install(&repo).unwrap();
        let content2 = fs::read_to_string(&path).unwrap();
        assert_eq!(content2.matches(BEGIN_MARKER).count(), 1);

        // Uninstall removes the now-bare hook file.
        assert!(uninstall(&repo).unwrap());
        assert!(!path.exists());
        // Second uninstall is a no-op.
        assert!(!uninstall(&repo).unwrap());
    }

    #[test]
    fn test_uninstall_preserves_other_hook_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(tmp.path()).unwrap();
        let dir = hooks_dir(&repo);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(HOOK_NAME);
        fs::write(&path, "#!/bin/sh\necho pre-existing\n").unwrap();

        install(&repo).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("echo pre-existing"));
        assert!(content.contains(BEGIN_MARKER));

        uninstall(&repo).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("echo pre-existing"), "user hook clobbered");
        assert!(!content.contains(BEGIN_MARKER));
    }
}
