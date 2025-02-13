use colored::*;
use git2::{DiffLineType, Error as GitError, Repository};

pub fn get_staged_changes(repo: &Repository) -> Result<String, GitError> {
    let mut opts = git2::DiffOptions::new();
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

    let diff = repo
        .diff_tree_to_index(Some(&tree), None, Some(&mut opts))
        .map_err(|e| GitError::from_str(&format!("Failed to get repository diff: {}", e)))?;

    let mut diff_str = String::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        match line.origin() {
            '+' | '-' | ' ' => {
                // Preserve the prefix character for additions, deletions, and context
                diff_str.push(line.origin());
                diff_str.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
            }
            _ => {
                // For headers and other lines, just add the content
                diff_str.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
            }
        }
        true
    })
    .map_err(|e| GitError::from_str(&format!("Failed to format diff: {}", e)))?;

    if diff_str.is_empty() {
        Err(GitError::from_str("No changes have been staged for commit"))
    } else {
        Ok(diff_str)
    }
}

fn has_unstaged_changes(repo: &Repository) -> Result<bool, GitError> {
    let diff = repo.diff_index_to_workdir(None, None)?;
    Ok(diff.stats()?.files_changed() > 0)
}

pub fn git_staged_changes(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
    let mut opts = git2::DiffOptions::new();
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

    let diff = repo
        .diff_tree_to_index(Some(&tree), None, Some(&mut opts))
        .map_err(|e| GitError::from_str(&format!("Failed to get repository diff: {}", e)))?;

    let stats = diff.stats()?;

    println!("\n{}", "Diff Statistics:".blue().bold());

    // Print the summary with colors
    let insertions = stats.insertions();
    let deletions = stats.deletions();
    println!(
        "{} files changed, {}(+) insertions, {}(-) deletions",
        stats.files_changed(),
        format!("{}", insertions).green(),
        format!("{}", deletions).red(),
    );

    // Print the per-file changes with visualization
    let mut format_opts = git2::DiffStatsFormat::empty();
    format_opts.insert(git2::DiffStatsFormat::FULL);
    format_opts.insert(git2::DiffStatsFormat::INCLUDE_SUMMARY);
    let changes_buf = stats.to_buf(format_opts, 80)?;

    // Find the longest filename for alignment
    let changes_str = String::from_utf8_lossy(&changes_buf);
    let max_filename_len = changes_str
        .lines()
        .filter(|line| line.contains('|'))
        .map(|line| line.splitn(2, '|').next().unwrap_or("").trim().len())
        .max()
        .unwrap_or(0);

    // Print aligned file changes
    for line in changes_str.lines() {
        if line.contains('|') {
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            if parts.len() == 2 {
                let (file, changes) = (parts[0].trim(), parts[1].trim());
                let count = changes.chars().filter(|&c| c == '+' || c == '-').count();

                // Extract the numeric count from the beginning of changes
                let num_count = changes.split_whitespace().next().unwrap_or("0");

                // Print the plus/minus visualization with colors
                print!(
                    "{:<width$} | {:>3} {:>3} ",
                    file,
                    count,
                    num_count,
                    width = max_filename_len
                );

                // Print each character with appropriate color
                for c in changes.chars().filter(|&c| c == '+' || c == '-') {
                    if c == '+' {
                        print!("{}", c.to_string().green());
                    } else {
                        print!("{}", c.to_string().red());
                    }
                }
                println!();
            }
        }
    }

    // Check for unstaged changes and warn the user
    if has_unstaged_changes(repo)? {
        println!("\n{}", "Warning:".yellow().bold());
        println!(
            "{}",
            "You have unstaged changes that won't be included in this commit.".yellow()
        );
        println!(
            "{}",
            "Use 'git add' to stage changes you want to include.".yellow()
        );
    }

    Ok(())
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
        let result = get_staged_changes(&repo);
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

        let changes = get_staged_changes(&repo).unwrap();
        assert!(changes.contains("Hello, World!"));
    }

    #[test]
    fn test_get_staged_changes_modified_file() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create and commit initial file
        create_and_stage_file(&repo, "test.txt", "Initial content");
        commit_all(&repo, "Initial commit");

        // Modify and stage the file
        create_and_stage_file(&repo, "test.txt", "Modified content");

        let changes = get_staged_changes(&repo).unwrap();
        assert!(changes.contains("Initial content"));
        assert!(changes.contains("Modified content"));
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

        // Should succeed and include warning about unstaged changes
        let result = git_staged_changes(&repo);
        assert!(result.is_ok());
    }
}
