use colored::*;
use git2::{Error as GitError, Repository, Sort};
use std::cmp;
use std::collections::HashSet;
use std::path::Path;

use crate::cmtignore::matches_pattern;

/// Stats about staged changes for display
#[derive(Debug, Clone)]
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub file_changes: Vec<(String, usize, usize)>, // (filename, adds, dels)
    pub skipped_files: Vec<(String, usize, usize)>, // Files exceeding max_file_lines threshold
    pub ignored_files: Vec<(String, usize, usize)>, // Files matched by .cmtignore
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

        // Collect all files for calculating column widths
        let all_files: Vec<_> = self
            .file_changes
            .iter()
            .chain(self.ignored_files.iter())
            .chain(self.skipped_files.iter())
            .collect();

        // Calculate column widths
        let max_name_len = all_files.iter().map(|(f, _, _)| f.len()).max().unwrap_or(0);
        let max_adds = all_files
            .iter()
            .map(|(_, a, _)| *a)
            .max()
            .unwrap_or(0)
            .max(self.insertions);
        let max_dels = all_files
            .iter()
            .map(|(_, _, d)| *d)
            .max()
            .unwrap_or(0)
            .max(self.deletions);

        // Calculate digit widths for alignment
        let adds_width = format!("{}", max_adds).len();
        let dels_width = format!("{}", max_dels).len();

        // Print header with +/- aligned to file columns
        // Header prefix: "Staged: N file(s) "
        let header_prefix = format!(
            "Staged: {} file{} ",
            self.files_changed,
            if self.files_changed == 1 { "" } else { "s" }
        );
        // File column width: "  " (indent) + filename + 4 spaces padding
        let file_col_width = 2 + max_name_len + 4;
        // Padding needed to align header + with file + column
        let header_pad = file_col_width.saturating_sub(header_prefix.len());

        print!("{}", "Staged:".blue());
        print!(
            " {} ",
            format!(
                "{} file{}",
                self.files_changed,
                if self.files_changed == 1 { "" } else { "s" }
            )
            .white()
        );
        // Align + column: header_pad spaces + adds_width + 1 for the '+' sign
        // Pad the string first, then colorize (ANSI codes break format width)
        print!(
            "{}",
            format!("{:>width$}", format!("+{}", self.insertions), width = header_pad + adds_width + 1).green()
        );
        // 3 spaces between + and - columns (print separately to avoid ANSI interference)
        print!("   ");
        println!(
            "{}",
            format!("{:>width$}", format!("-{}", self.deletions), width = dels_width + 1).red()
        );

        // Print regular file list
        for (file, adds, dels) in &self.file_changes {
            print!("  {:<width$}", file.white(), width = max_name_len + 4);
            if *adds > 0 {
                print!(
                    "{}",
                    format!("{:>width$}", format!("+{}", adds), width = adds_width + 1).green()
                );
            } else {
                print!("{:>width$}", "", width = adds_width + 1);
            }
            if *dels > 0 {
                // Print spacing separately to avoid ANSI code interference
                print!("   ");
                println!(
                    "{}",
                    format!("{:>width$}", format!("-{}", dels), width = dels_width + 1).red()
                );
            } else {
                println!();
            }
        }

        // Print ignored files (auto-skipped + .cmtignore) - dimmed with ~ marker
        for (file, adds, dels) in &self.ignored_files {
            print!("  {:<width$}", file.dimmed(), width = max_name_len + 4);
            if *adds > 0 {
                print!(
                    "{}",
                    format!("{:>width$}", format!("+{}", adds), width = adds_width + 1).dimmed()
                );
            } else {
                print!("{:>width$}", "", width = adds_width + 1);
            }
            if *dels > 0 {
                print!("   ");
                print!(
                    "{}",
                    format!("{:>width$}", format!("-{}", dels), width = dels_width + 1).dimmed()
                );
            } else {
                print!("{:>width$}", "", width = dels_width + 4);
            }
            println!("{}", "  ~".dimmed());
        }

        // Print skipped files (exceeding threshold) - dimmed with ~ marker
        for (file, adds, dels) in &self.skipped_files {
            print!("  {:<width$}", file.dimmed(), width = max_name_len + 4);
            if *adds > 0 {
                print!(
                    "{}",
                    format!("{:>width$}", format!("+{}", adds), width = adds_width + 1).dimmed()
                );
            } else {
                print!("{:>width$}", "", width = adds_width + 1);
            }
            if *dels > 0 {
                print!("   ");
                print!(
                    "{}",
                    format!("{:>width$}", format!("-{}", dels), width = dels_width + 1).dimmed()
                );
            } else {
                print!("{:>width$}", "", width = dels_width + 4);
            }
            println!("{}", "  ~".dimmed());
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

/// Get the current branch name
pub fn get_current_branch(repo: &Repository) -> Option<String> {
    repo.head().ok().and_then(|head| {
        if head.is_branch() {
            head.shorthand().map(|s| s.to_string())
        } else {
            // Detached HEAD - return short commit hash
            head.peel_to_commit()
                .ok()
                .map(|c| format!("detached@{}", &c.id().to_string()[..7]))
        }
    })
}

/// Get the first N lines of the README for project context
pub fn get_readme_excerpt(repo: &Repository, max_lines: usize) -> Option<String> {
    let workdir = repo.workdir()?;

    // Try common README filenames
    let readme_names = [
        "README.md",
        "README.MD",
        "readme.md",
        "README",
        "README.txt",
    ];

    for name in &readme_names {
        let path = workdir.join(name);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let lines: Vec<&str> = content.lines().take(max_lines).collect();
                if !lines.is_empty() {
                    return Some(lines.join("\n"));
                }
            }
        }
    }

    None
}

pub fn get_staged_changes(
    repo: &Repository,
    context_lines: u32,
    max_lines_per_file: usize,
    max_line_width: usize,
    max_file_lines: usize,
    cmtignore_patterns: &[String],
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

    // Get stats (for reference, though we calculate our own for accurate filtering)
    let _git_stats = diff.stats()?;

    // Collect per-file stats using Patch API for accurate line counts
    // Separate into regular files, ignored files, and skipped files
    let mut file_changes: Vec<(String, usize, usize)> = Vec::new();
    let mut ignored_files: Vec<(String, usize, usize)> = Vec::new();
    let mut skipped_files: Vec<(String, usize, usize)> = Vec::new();

    for delta_idx in 0..diff.deltas().len() {
        if let Ok(Some(patch)) = git2::Patch::from_diff(&diff, delta_idx) {
            let file_path = patch
                .delta()
                .new_file()
                .path()
                .or_else(|| patch.delta().old_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            if file_path.is_empty() {
                continue;
            }

            // line_stats returns (context_lines, additions, deletions)
            let (_, additions, deletions) = patch.line_stats().unwrap_or((0, 0, 0));
            let total_lines = additions + deletions;

            // Check if file is auto-skipped (lock files, images, build artifacts)
            let file_path_obj = Path::new(&file_path);
            let is_auto_skipped = is_skippable(file_path_obj);

            // Check if file matches any .cmtignore pattern
            let is_ignored = cmtignore_patterns
                .iter()
                .any(|pattern| matches_pattern(&file_path, pattern));

            if is_auto_skipped || is_ignored {
                // Auto-skipped and .cmtignore files go to ignored_files
                ignored_files.push((file_path, additions, deletions));
            } else if max_file_lines > 0 && total_lines > max_file_lines {
                // File exceeds threshold (only check if threshold > 0)
                skipped_files.push((file_path, additions, deletions));
            } else {
                file_changes.push((file_path, additions, deletions));
            }
        }
    }

    // Calculate total insertions/deletions across ALL files (including ignored/skipped)
    let total_insertions: usize = file_changes
        .iter()
        .chain(ignored_files.iter())
        .chain(skipped_files.iter())
        .map(|(_, a, _)| a)
        .sum();
    let total_deletions: usize = file_changes
        .iter()
        .chain(ignored_files.iter())
        .chain(skipped_files.iter())
        .map(|(_, _, d)| d)
        .sum();
    let total_files = file_changes.len() + ignored_files.len() + skipped_files.len();

    let stats = DiffStats {
        files_changed: total_files,
        insertions: total_insertions,
        deletions: total_deletions,
        file_changes,
        skipped_files,
        ignored_files,
        has_unstaged: has_unstaged_changes(repo).unwrap_or(false),
    };

    // Adaptive trimming: only tighten for very large diffs (Gemini Flash supports 1M tokens)
    let very_large_diff = stats.files_changed > 100 || (stats.insertions + stats.deletions) > 20000;
    let effective_context_lines = if very_large_diff {
        // Still keep reasonable context even for massive diffs
        context_lines.clamp(8, 15)
    } else {
        context_lines
    };
    let effective_max_lines_per_file = if very_large_diff {
        cmp::min(max_lines_per_file, 500)
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

    // Build set of files to exclude from diff text (ignored + skipped)
    let excluded_files: HashSet<String> = stats
        .ignored_files
        .iter()
        .chain(stats.skipped_files.iter())
        .map(|(f, _, _)| f.clone())
        .collect();

    // Build diff text
    let mut diff_str = String::new();
    let mut line_count = 0;
    let mut truncated = false;

    diff.print(git2::DiffFormat::Patch, |delta, _, line| {
        let file_path = delta
            .new_file()
            .path()
            .unwrap_or_else(|| std::path::Path::new(""));

        // Skip .lock files and other auto-skippable files
        if is_skippable(file_path) {
            return true;
        }

        // Skip files that are ignored or exceed threshold
        let file_path_str = file_path.to_string_lossy();
        if excluded_files.contains(file_path_str.as_ref()) {
            return true;
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
        let result = get_staged_changes(&repo, 0, 100, 300, 0, &[]);
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

        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &[]).unwrap();
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

        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &[]).unwrap();
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
        let result = get_staged_changes(&repo, 3, 100, 300, 0, &[]).unwrap();
        assert!(result.stats.has_unstaged);
    }

    #[test]
    fn test_exclude_lock_files_from_diff() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and stage a .lock file
        create_and_stage_file(&repo, "test.lock", "This is a lock file.");

        // Create and stage a regular file
        create_and_stage_file(&repo, "test.txt", "This is a regular file.");

        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &[]).unwrap();

        // Lock file should be in ignored_files (auto-skipped)
        assert_eq!(staged.stats.ignored_files.len(), 1);
        assert_eq!(staged.stats.ignored_files[0].0, "test.lock");

        // Regular file should be in file_changes
        assert_eq!(staged.stats.file_changes.len(), 1);
        assert_eq!(staged.stats.file_changes[0].0, "test.txt");

        // Total stats should include both files
        assert_eq!(staged.stats.files_changed, 2);

        // Diff text (sent to LLM) should only contain regular file
        assert!(!staged.diff_text.contains("This is a lock file."));
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
        let staged = get_staged_changes(&repo, 0, max_lines_per_file, 300, 0, &[]).unwrap();

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
        let staged = get_staged_changes(&repo, 0, 100, max_line_width, 0, &[]).unwrap();

        // Assert that the line is truncated to max_line_width
        assert!(staged.diff_text.contains(&long_line[..max_line_width]));
        assert!(staged.diff_text.contains("..."));
    }

    #[test]
    fn test_file_changes_new_file() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and stage a new file with 5 lines
        let content = "line1\nline2\nline3\nline4\nline5";
        create_and_stage_file(&repo, "test.txt", content);

        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &[]).unwrap();

        // Check overall stats
        assert_eq!(staged.stats.files_changed, 1);
        assert_eq!(staged.stats.insertions, 5);
        assert_eq!(staged.stats.deletions, 0);

        // Check per-file stats
        assert_eq!(staged.stats.file_changes.len(), 1);
        let (file, adds, dels) = &staged.stats.file_changes[0];
        assert_eq!(file, "test.txt");
        assert_eq!(*adds, 5);
        assert_eq!(*dels, 0);
    }

    #[test]
    fn test_file_changes_modified_file() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and commit initial file with 3 lines
        create_and_stage_file(&repo, "test.txt", "line1\nline2\nline3");
        commit_all(&repo, "Initial commit");

        // Modify file: change line2, add line4
        create_and_stage_file(&repo, "test.txt", "line1\nmodified\nline3\nline4");

        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &[]).unwrap();

        // Check per-file stats - should have 2 insertions (modified, line4) and 1 deletion (line2)
        assert_eq!(staged.stats.file_changes.len(), 1);
        let (file, adds, dels) = &staged.stats.file_changes[0];
        assert_eq!(file, "test.txt");
        assert_eq!(*adds, 2);
        assert_eq!(*dels, 1);
    }

    #[test]
    fn test_file_changes_multiple_files() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create initial commit
        create_and_stage_file(&repo, "file1.txt", "a\nb\nc");
        create_and_stage_file(&repo, "file2.txt", "x\ny");
        commit_all(&repo, "Initial commit");

        // Stage changes to multiple files
        create_and_stage_file(&repo, "file1.txt", "a\nmodified\nc\nd"); // +2 -1
        create_and_stage_file(&repo, "file2.txt", "x"); // -1
        create_and_stage_file(&repo, "file3.txt", "new1\nnew2\nnew3"); // +3

        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &[]).unwrap();

        // Check overall stats
        assert_eq!(staged.stats.files_changed, 3);

        // Check per-file stats
        assert_eq!(staged.stats.file_changes.len(), 3);

        // Find each file's stats (order may vary)
        let file1_stats = staged
            .stats
            .file_changes
            .iter()
            .find(|(f, _, _)| f == "file1.txt");
        let file2_stats = staged
            .stats
            .file_changes
            .iter()
            .find(|(f, _, _)| f == "file2.txt");
        let file3_stats = staged
            .stats
            .file_changes
            .iter()
            .find(|(f, _, _)| f == "file3.txt");

        assert!(file1_stats.is_some());
        assert!(file2_stats.is_some());
        assert!(file3_stats.is_some());

        let (_, adds1, dels1) = file1_stats.unwrap();
        assert_eq!(*adds1, 2);
        assert_eq!(*dels1, 1);

        let (_, adds2, dels2) = file2_stats.unwrap();
        assert_eq!(*adds2, 0);
        assert_eq!(*dels2, 1);

        let (_, adds3, dels3) = file3_stats.unwrap();
        assert_eq!(*adds3, 3);
        assert_eq!(*dels3, 0);
    }

    #[test]
    fn test_file_changes_sum_matches_total() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create initial commit
        create_and_stage_file(&repo, "a.txt", "1\n2\n3\n4\n5");
        create_and_stage_file(&repo, "b.txt", "a\nb\nc");
        commit_all(&repo, "Initial commit");

        // Make various changes
        create_and_stage_file(&repo, "a.txt", "1\nchanged\n3\n4\n5\n6\n7"); // +3 -1
        create_and_stage_file(&repo, "b.txt", "a"); // -2
        create_and_stage_file(&repo, "c.txt", "new\nfile"); // +2

        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &[]).unwrap();

        // Sum up per-file stats
        let total_adds: usize = staged.stats.file_changes.iter().map(|(_, a, _)| a).sum();
        let total_dels: usize = staged.stats.file_changes.iter().map(|(_, _, d)| d).sum();

        // Verify they match overall stats
        assert_eq!(total_adds, staged.stats.insertions);
        assert_eq!(total_dels, staged.stats.deletions);
    }

    #[test]
    fn test_discover_repo_from_subdirectory() {
        let (temp_dir, repo) = setup_test_repo();

        // Create a nested subdirectory structure
        let subdir = temp_dir.path().join("src").join("deeply").join("nested");
        std::fs::create_dir_all(&subdir).unwrap();

        // Stage a file so we have something to work with
        create_and_stage_file(&repo, "test.txt", "Hello from root");

        // Discover the repo from the nested subdirectory
        let discovered = Repository::discover(&subdir).unwrap();

        // Verify we found the same repository (compare workdir paths)
        assert_eq!(
            discovered.workdir().unwrap().canonicalize().unwrap(),
            repo.workdir().unwrap().canonicalize().unwrap()
        );

        // Verify we can still access staged changes from the discovered repo
        let staged = get_staged_changes(&discovered, 0, 100, 300, 0, &[]).unwrap();
        assert!(staged.diff_text.contains("Hello from root"));
    }

    #[test]
    fn test_cmtignore_pattern_matching() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and stage files
        create_and_stage_file(&repo, "regular.txt", "regular content");
        create_and_stage_file(&repo, "ignored.sql", "ignored content");

        // Use cmtignore pattern to ignore .sql files
        let patterns = vec!["*.sql".to_string()];
        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &patterns).unwrap();

        // Regular file should be in file_changes
        assert_eq!(staged.stats.file_changes.len(), 1);
        assert_eq!(staged.stats.file_changes[0].0, "regular.txt");

        // SQL file should be in ignored_files
        assert_eq!(staged.stats.ignored_files.len(), 1);
        assert_eq!(staged.stats.ignored_files[0].0, "ignored.sql");

        // Total stats should include BOTH files (ignored files are only skipped for analysis)
        assert_eq!(staged.stats.files_changed, 2);
        assert_eq!(staged.stats.insertions, 2); // 1 line each

        // Diff text (sent to LLM) should only contain regular file content
        assert!(staged.diff_text.contains("regular content"));
        assert!(!staged.diff_text.contains("ignored content"));
    }

    #[test]
    fn test_max_file_lines_threshold() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create a small file (under threshold)
        create_and_stage_file(&repo, "small.txt", "line1\nline2\nline3");

        // Create a large file (over threshold of 5 lines)
        let large_content = (0..10).map(|i| format!("line{}", i)).collect::<Vec<_>>().join("\n");
        create_and_stage_file(&repo, "large.txt", &large_content);

        // Use max_file_lines of 5
        let staged = get_staged_changes(&repo, 0, 100, 300, 5, &[]).unwrap();

        // Small file should be in file_changes
        assert_eq!(staged.stats.file_changes.len(), 1);
        assert_eq!(staged.stats.file_changes[0].0, "small.txt");

        // Large file should be in skipped_files
        assert_eq!(staged.stats.skipped_files.len(), 1);
        assert_eq!(staged.stats.skipped_files[0].0, "large.txt");

        // Total stats should include BOTH files (skipped files are only skipped for analysis)
        assert_eq!(staged.stats.files_changed, 2);
        assert_eq!(staged.stats.insertions, 13); // 3 + 10 lines

        // Diff text (sent to LLM) should only contain small file content
        assert!(staged.diff_text.contains("line1"));
        assert!(!staged.diff_text.contains("line9"));
    }

    #[test]
    fn test_max_file_lines_zero_disables_check() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create a large file
        let large_content = (0..100).map(|i| format!("line{}", i)).collect::<Vec<_>>().join("\n");
        create_and_stage_file(&repo, "large.txt", &large_content);

        // Use max_file_lines of 0 (disabled)
        let staged = get_staged_changes(&repo, 0, 100, 300, 0, &[]).unwrap();

        // Large file should be in file_changes (not skipped)
        assert_eq!(staged.stats.file_changes.len(), 1);
        assert_eq!(staged.stats.skipped_files.len(), 0);
    }
}
