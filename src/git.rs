use colored::*;
use git2::{Error as GitError, Repository, Sort};
use std::cmp;
use std::path::Path;

/// Stats about staged changes for display
#[derive(Debug, Clone)]
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub file_changes: Vec<(String, usize, usize)>, // (filename, adds, dels)
    pub has_unstaged: bool,
}

impl DiffStats {
    /// Print the stats in a compact format
    pub fn print(&self) {
        println!();

        // Warn about unstaged changes prominently at the top
        if self.has_unstaged {
            println!(
                "{}",
                "⚠  You have unstaged changes that won't be included in this commit"
                    .yellow()
                    .bold()
            );
            println!();
        }

        // Print compact header
        print!(
            "{} {} ",
            "Staged:".blue(),
            format!(
                "{} file{}",
                self.files_changed,
                if self.files_changed == 1 { "" } else { "s" }
            )
            .white()
        );
        if self.insertions > 0 {
            print!("{} ", format!("+{}", self.insertions).green());
        }
        if self.deletions > 0 {
            print!("{}", format!("-{}", self.deletions).red());
        }
        println!();

        // Print file list (compact)
        let max_len = self
            .file_changes
            .iter()
            .map(|(f, _, _)| f.len())
            .max()
            .unwrap_or(0);

        for (file, adds, dels) in &self.file_changes {
            print!("  {:<width$}", file.white(), width = max_len + 2);
            if *adds > 0 {
                print!("{}", format!("+{:<3}", adds).green());
            } else {
                print!("    ");
            }
            if *dels > 0 {
                print!("{}", format!("-{}", dels).red());
            }
            println!();
        }
        println!(); // Space before next section
    }
}

/// Result of getting staged changes - includes both diff text and stats
#[derive(Debug)]
pub struct StagedChanges {
    pub diff_text: String,
    pub stats: DiffStats,
}

fn is_skippable(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let path_str = path.to_string_lossy().to_lowercase();

    // Lock and dependency snapshot files
    if ext == "lock"
        || name == "package-lock.json"
        || name == "pnpm-lock.yaml"
        || name == "yarn.lock"
        || name == "cargo.lock"
    {
        return true;
    }

    // Large generated / compiled assets commonly not useful for commit intent
    if ext == "map" || ext == "min.js" || ext == "min.css" {
        return true;
    }

    // Binary/media assets that bloat prompts; the model can't interpret them meaningfully
    if matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "ico" | "svg"
    ) {
        return true;
    }

    // Skip obvious build artifacts if staged
    if path_str.starts_with("dist/") || path_str.starts_with("build/") {
        return true;
    }

    false
}

pub fn get_recent_commits(repo: &Repository, count: usize) -> Result<String, GitError> {
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TIME)?;
    revwalk.push_head()?;

    let mut commit_messages = String::new();

    for (i, oid) in revwalk.take(count).enumerate() {
        if let Ok(oid) = oid {
            if let Ok(commit) = repo.find_commit(oid) {
                commit_messages.push_str(&format!(
                    "[{}] {}\n",
                    i + 1,
                    commit.message().unwrap_or("")
                ));
            }
        }
    }

    Ok(commit_messages)
}

pub fn get_staged_changes(
    repo: &Repository,
    context_lines: u32,
    max_lines_per_file: usize,
    max_line_width: usize,
) -> Result<StagedChanges, GitError> {
    let mut opts = git2::DiffOptions::new();
    opts.context_lines(context_lines);

    let tree = match repo.head().and_then(|head| head.peel_to_tree()) {
        Ok(tree) => tree,
        Err(_) => {
            // If there's no HEAD (new repo), use an empty tree
            repo.treebuilder(None)
                .and_then(|builder| builder.write())
                .and_then(|oid| repo.find_tree(oid))
                .map_err(|e| GitError::from_str(&format!("Failed to create empty tree: {}", e)))?
        }
    };

    // First pass: build diff and get stats
    let diff = repo
        .diff_tree_to_index(Some(&tree), None, Some(&mut opts))
        .map_err(|e| GitError::from_str(&format!("Failed to get repository diff: {}", e)))?;

    // Get stats in the same pass
    let git_stats = diff.stats()?;
    let mut format_opts = git2::DiffStatsFormat::empty();
    format_opts.insert(git2::DiffStatsFormat::FULL);
    let changes_buf = git_stats.to_buf(format_opts, 80)?;
    let changes_str = String::from_utf8_lossy(&changes_buf);

    let file_changes: Vec<(String, usize, usize)> = changes_str
        .lines()
        .filter(|line| line.contains('|'))
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            if parts.len() == 2 {
                let file = parts[0].trim().to_string();
                let changes = parts[1].trim();
                let adds = changes.chars().filter(|&c| c == '+').count();
                let dels = changes.chars().filter(|&c| c == '-').count();
                Some((file, adds, dels))
            } else {
                None
            }
        })
        .collect();

    let stats = DiffStats {
        files_changed: git_stats.files_changed(),
        insertions: git_stats.insertions(),
        deletions: git_stats.deletions(),
        file_changes,
        has_unstaged: has_unstaged_changes(repo).unwrap_or(false),
    };

    // Adaptive trimming: tighten context/line caps only for large diffs
    let large_diff =
        stats.files_changed > 40 || (stats.insertions + stats.deletions) > 4000;
    let effective_context_lines = if large_diff {
        // Keep enough context to preserve meaning, but trim heavy payloads
        cmp::max(3, cmp::min(context_lines, 6))
    } else {
        context_lines
    };
    let effective_max_lines_per_file = if large_diff {
        cmp::min(max_lines_per_file, 200)
    } else {
        max_lines_per_file
    };

    // If we tightened context lines, rebuild diff with the smaller context for printing
    let diff = if effective_context_lines != context_lines {
        let mut opts = git2::DiffOptions::new();
        opts.context_lines(effective_context_lines);
        repo.diff_tree_to_index(Some(&tree), None, Some(&mut opts))
            .map_err(|e| GitError::from_str(&format!("Failed to get repository diff: {}", e)))?
    } else {
        diff
    };

    // Build diff text
    let mut diff_str = String::new();
    let mut line_count = 0;
    let mut truncated = false;

    diff.print(git2::DiffFormat::Patch, |delta, _, line| {
        let file_path = delta
            .new_file()
            .path()
            .unwrap_or_else(|| std::path::Path::new(""));
        if is_skippable(file_path) {
            return true; // Skip .lock files
        }

        if line_count < effective_max_lines_per_file {
            match line.origin() {
                '+' | '-' | ' ' => {
                    // Preserve the prefix character for additions, deletions, and context
                    diff_str.push(line.origin());
                    let line_content = std::str::from_utf8(line.content()).unwrap_or("binary");
                    if line_content.len() > max_line_width {
                        diff_str.push_str(&line_content[..max_line_width]);
                        diff_str.push_str("...");
                    } else {
                        diff_str.push_str(line_content);
                    }
                    line_count += 1; // Increment line count only for content lines
                }
                _ => {
                    // For headers and other lines, just add the content
                    diff_str.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
                }
            }
        } else if !truncated {
            truncated = true;
            diff_str.push_str("\n[Note: Diff output truncated to max lines per file.]");
        }
        true
    })
    .map_err(|e| GitError::from_str(&format!("Failed to format diff: {}", e)))?;

    if diff_str.is_empty() {
        Err(GitError::from_str("No changes have been staged for commit"))
    } else {
        Ok(StagedChanges {
            diff_text: diff_str,
            stats,
        })
    }
}

fn has_unstaged_changes(repo: &Repository) -> Result<bool, GitError> {
    let diff = repo.diff_index_to_workdir(None, None)?;
    Ok(diff.stats()?.files_changed() > 0)
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();

        // Configure test user
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        (temp_dir, repo)
    }

    fn create_and_stage_file(repo: &Repository, name: &str, content: &str) {
        let path = repo.workdir().unwrap().join(name);
        let mut file = File::create(path).unwrap();
        writeln!(file, "{}", content).unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new(name)).unwrap();
        index.write().unwrap();
    }

    fn commit_all(repo: &Repository, message: &str) {
        let mut index = repo.index().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let sig = repo.signature().unwrap();
        if let Ok(parent) = repo.head().and_then(|h| h.peel_to_commit()) {
            repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
                .unwrap();
        } else {
            repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])
                .unwrap();
        }
    }

    #[test]
    fn test_get_staged_changes_empty_repo() {
        let (_temp_dir, repo) = setup_test_repo();
        let result = get_staged_changes(&repo, 0, 100, 300);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message(),
            "No changes have been staged for commit"
        );
    }

    #[test]
    fn test_get_staged_changes_new_file() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and stage a new file
        create_and_stage_file(&repo, "test.txt", "Hello, World!");

        let staged = get_staged_changes(&repo, 0, 100, 300).unwrap();
        assert!(staged.diff_text.contains("Hello, World!"));
    }

    #[test]
    fn test_get_staged_changes_modified_file() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and commit initial file
        create_and_stage_file(&repo, "test.txt", "Initial content");
        commit_all(&repo, "Initial commit");

        // Modify and stage the file
        create_and_stage_file(&repo, "test.txt", "Modified content");

        let staged = get_staged_changes(&repo, 0, 100, 300).unwrap();
        assert!(staged.diff_text.contains("Initial content"));
        assert!(staged.diff_text.contains("Modified content"));
    }

    #[test]
    fn test_has_unstaged_changes() {
        let (_temp_dir, repo) = setup_test_repo();

        // Initially should have no unstaged changes
        assert!(!has_unstaged_changes(&repo).unwrap());

        // Create and stage a file first
        create_and_stage_file(&repo, "test.txt", "Initial content");
        commit_all(&repo, "Initial commit");

        // Modify the file without staging it
        let path = repo.workdir().unwrap().join("test.txt");
        let mut file = File::create(path).unwrap();
        writeln!(file, "Modified content").unwrap();

        // Should now detect unstaged changes
        assert!(has_unstaged_changes(&repo).unwrap());
    }

    #[test]
    fn test_show_git_diff_with_unstaged_changes() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and stage a file
        create_and_stage_file(&repo, "staged.txt", "Staged content");
        commit_all(&repo, "Initial commit");

        // Modify the file without staging changes
        let path = repo.workdir().unwrap().join("staged.txt");
        let mut file = File::create(path).unwrap();
        writeln!(file, "Modified unstaged content").unwrap();

        // Create another staged file
        create_and_stage_file(&repo, "new-staged.txt", "New staged content");

        // Should succeed and detect unstaged changes
        let result = get_staged_changes(&repo, 3, 100, 300).unwrap();
        assert!(result.stats.has_unstaged);
    }

    #[test]
    fn test_exclude_lock_files_from_diff() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and stage a .lock file
        create_and_stage_file(&repo, "test.lock", "This is a lock file.");

        // Create and stage a regular file
        create_and_stage_file(&repo, "test.txt", "This is a regular file.");

        let staged = get_staged_changes(&repo, 0, 100, 300).unwrap();

        // Assert that the .lock file content is not in the diff
        assert!(!staged.diff_text.contains("This is a lock file."));

        // Assert that the regular file content is in the diff
        assert!(staged.diff_text.contains("This is a regular file."));
    }

    #[test]
    fn test_max_lines_per_file_limit() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and stage a file with more lines than the max_lines_per_file limit
        let mut content = String::new();
        for i in 0..600 {
            content.push_str(&format!("Line {}\n", i));
        }
        create_and_stage_file(&repo, "test.txt", &content);

        // Set max_lines_per_file to 10 for testing
        let max_lines_per_file = 10;
        let staged = get_staged_changes(&repo, 0, max_lines_per_file, 300).unwrap();

        // Assert that the diff output does not exceed the max_lines_per_file limit
        // Allow extra lines for headers and metadata
        // let allowed_extra_lines = 6; // Adjust this number based on typical header lines

        // Assert that the truncation note is included
        assert!(staged
            .diff_text
            .contains("[Note: Diff output truncated to max lines per file.]"));
        assert!(staged
            .diff_text
            .contains(&format!("+Line {}", max_lines_per_file - 1)));
        assert!(!staged
            .diff_text
            .contains(&format!("+Line {}", max_lines_per_file)));
    }

    #[test]
    fn test_max_line_width() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and stage a file with a long line
        let long_line = "a".repeat(400);
        create_and_stage_file(&repo, "test.txt", &long_line);

        // Set max_line_width to 100 for testing
        let max_line_width = 100;
        let staged = get_staged_changes(&repo, 0, 100, max_line_width).unwrap();

        // Assert that the line is truncated to max_line_width
        assert!(staged.diff_text.contains(&long_line[..max_line_width]));
        assert!(staged.diff_text.contains("..."));
    }
}
