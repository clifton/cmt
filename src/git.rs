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

    let mut diff_output = Vec::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        match line.origin_value() {
            DiffLineType::Addition | DiffLineType::Deletion | DiffLineType::Context => {
                diff_output.extend_from_slice(line.content());
            }
            _ => {}
        }
        true
    })
    .map_err(|e| GitError::from_str(&format!("Failed to format diff: {}", e)))?;

    let diff_str = String::from_utf8_lossy(&diff_output).to_string();

    if diff_str.is_empty() {
        Err(GitError::from_str("No changes have been staged for commit"))
    } else {
        Ok(diff_str)
    }
}

pub fn show_git_diff(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
    let diff = repo.diff_index_to_workdir(None, None)?;
    let stats = diff.stats()?;
    println!("\n{}", "Diff Statistics:".blue().bold());
    println!("Files changed: {}", stats.files_changed());
    println!("Insertions: {}", stats.insertions());
    println!("Deletions: {}", stats.deletions());
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
    fn test_show_git_diff() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create initial file
        create_and_stage_file(&repo, "test.txt", "Initial content");
        commit_all(&repo, "Initial commit");

        // Modify the file but don't stage it
        let path = repo.workdir().unwrap().join("test.txt");
        let mut file = File::create(path).unwrap();
        writeln!(file, "Modified content").unwrap();

        // Capture stdout to verify the output
        let result = show_git_diff(&repo);
        assert!(result.is_ok());
    }
}
