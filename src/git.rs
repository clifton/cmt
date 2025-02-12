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
