//! .cmtignore file support for excluding files from commit message generation

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::config::defaults::CMTIGNORE_FILENAME;

/// Load patterns from .cmtignore file in the repository root
///
/// Returns an empty vector if the file doesn't exist or can't be read.
/// Lines starting with # are treated as comments and ignored.
/// Empty lines are also ignored.
pub fn load_cmtignore(repo_root: &Path) -> Vec<String> {
    let cmtignore_path = repo_root.join(CMTIGNORE_FILENAME);

    if !cmtignore_path.exists() {
        return Vec::new();
    }

    match fs::read_to_string(&cmtignore_path) {
        Ok(content) => content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| line.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Append file patterns to .cmtignore
///
/// Creates the file if it doesn't exist. Adds patterns on new lines.
/// Returns Ok(()) on success or an io::Error on failure.
pub fn append_to_cmtignore(repo_root: &Path, files: &[String]) -> io::Result<()> {
    let cmtignore_path = repo_root.join(CMTIGNORE_FILENAME);

    // Check if file exists and if it ends with a newline
    let needs_leading_newline = if cmtignore_path.exists() {
        let content = fs::read_to_string(&cmtignore_path)?;
        !content.is_empty() && !content.ends_with('\n')
    } else {
        false
    };

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cmtignore_path)?;

    // Add leading newline if needed
    if needs_leading_newline {
        writeln!(file)?;
    }

    // Write each file pattern on its own line
    for pattern in files {
        writeln!(file, "{}", pattern)?;
    }

    Ok(())
}

/// Check if a file path matches a .cmtignore pattern
///
/// Supports simple glob patterns:
/// - `*` matches any sequence of characters (except `/`)
/// - `**` matches any sequence of characters (including `/`)
/// - Exact matches work as expected
pub fn matches_pattern(path: &str, pattern: &str) -> bool {
    // Normalize path separators
    let path = path.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");

    // Handle ** glob (matches any path depth)
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1].trim_start_matches('/');

            // Check if path starts with prefix (if any) and ends with suffix (if any)
            let matches_prefix = prefix.is_empty() || path.starts_with(prefix);

            // For suffix, we need to handle patterns like "*.tsx" - match against filename
            let matches_suffix = if suffix.is_empty() {
                true
            } else if let Some(ext_pattern) = suffix.strip_prefix('*') {
                // Suffix like "*.tsx" - check if any path component matches
                // ext_pattern is e.g., ".tsx"
                path.ends_with(ext_pattern)
            } else {
                path.ends_with(suffix)
            };

            return matches_prefix && matches_suffix;
        }
    }

    // Handle * glob (matches within single path component)
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];

            // For single *, don't match across directory separators
            let matches_prefix = path.starts_with(prefix);
            let matches_suffix = path.ends_with(suffix);

            if matches_prefix && matches_suffix {
                // Check that the middle part doesn't contain /
                let middle_start = prefix.len();
                let middle_end = path.len().saturating_sub(suffix.len());
                if middle_start <= middle_end {
                    let middle = &path[middle_start..middle_end];
                    return !middle.contains('/');
                }
            }
            return false;
        }
    }

    // Exact match
    path == pattern
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_cmtignore_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let patterns = load_cmtignore(temp_dir.path());
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_load_cmtignore_with_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let cmtignore_path = temp_dir.path().join(CMTIGNORE_FILENAME);

        fs::write(
            &cmtignore_path,
            "# Comment line\n\nmigrations/*.sql\n*.generated.ts\ndist/**\n",
        )
        .unwrap();

        let patterns = load_cmtignore(temp_dir.path());
        assert_eq!(patterns.len(), 3);
        assert_eq!(patterns[0], "migrations/*.sql");
        assert_eq!(patterns[1], "*.generated.ts");
        assert_eq!(patterns[2], "dist/**");
    }

    #[test]
    fn test_append_to_cmtignore_new_file() {
        let temp_dir = TempDir::new().unwrap();

        append_to_cmtignore(
            temp_dir.path(),
            &["file1.sql".to_string(), "file2.sql".to_string()],
        )
        .unwrap();

        let content = fs::read_to_string(temp_dir.path().join(CMTIGNORE_FILENAME)).unwrap();
        assert_eq!(content, "file1.sql\nfile2.sql\n");
    }

    #[test]
    fn test_append_to_cmtignore_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let cmtignore_path = temp_dir.path().join(CMTIGNORE_FILENAME);

        fs::write(&cmtignore_path, "existing.txt\n").unwrap();

        append_to_cmtignore(temp_dir.path(), &["new.txt".to_string()]).unwrap();

        let content = fs::read_to_string(&cmtignore_path).unwrap();
        assert_eq!(content, "existing.txt\nnew.txt\n");
    }

    #[test]
    fn test_matches_pattern_exact() {
        assert!(matches_pattern(
            "migrations/schema.sql",
            "migrations/schema.sql"
        ));
        assert!(!matches_pattern(
            "migrations/schema.sql",
            "migrations/other.sql"
        ));
    }

    #[test]
    fn test_matches_pattern_single_star() {
        assert!(matches_pattern("file.generated.ts", "*.generated.ts"));
        assert!(matches_pattern("foo.generated.ts", "*.generated.ts"));
        assert!(!matches_pattern("src/file.generated.ts", "*.generated.ts"));

        assert!(matches_pattern("migrations/schema.sql", "migrations/*.sql"));
        assert!(matches_pattern("migrations/data.sql", "migrations/*.sql"));
        assert!(!matches_pattern("other/schema.sql", "migrations/*.sql"));
    }

    #[test]
    fn test_matches_pattern_double_star() {
        assert!(matches_pattern("dist/file.js", "dist/**"));
        assert!(matches_pattern("dist/sub/file.js", "dist/**"));
        assert!(matches_pattern("dist/a/b/c/file.js", "dist/**"));
        assert!(!matches_pattern("src/file.js", "dist/**"));

        assert!(matches_pattern("src/components/Button.tsx", "**/*.tsx"));
        assert!(matches_pattern("Button.tsx", "**/*.tsx"));
    }
}
