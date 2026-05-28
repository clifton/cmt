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

/// Check if a file path matches a .cmtignore pattern.
///
/// Patterns are matched component-by-component (split on `/`):
/// - `*` matches any run of characters within a single path component (not `/`)
/// - `?` matches any single character within a component
/// - `**` matches zero or more whole path components (any depth)
/// - everything else is a literal match
///
/// This supports multiple wildcards in one pattern (e.g. `src/*/*.rs`,
/// `**/*.test.tsx`) and anchors `**` on component boundaries, so `**/foo.tsx`
/// matches `a/b/foo.tsx` but not `barfoo.tsx`.
pub fn matches_pattern(path: &str, pattern: &str) -> bool {
    let path = path.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");

    let pat: Vec<&str> = pattern.split('/').collect();
    let segs: Vec<&str> = path.split('/').collect();
    match_components(&pat, &segs)
}

/// Match pattern components against path components. A `**` component matches
/// zero or more path components; any other component is matched against a single
/// path component via [`segment_match`].
fn match_components(pat: &[&str], path: &[&str]) -> bool {
    match pat.split_first() {
        None => path.is_empty(),
        Some((&first, rest)) => {
            if first == "**" {
                // Try consuming 0..=path.len() leading components.
                (0..=path.len()).any(|i| match_components(rest, &path[i..]))
            } else {
                match path.split_first() {
                    Some((&head, tail)) if segment_match(first, head) => {
                        match_components(rest, tail)
                    }
                    _ => false,
                }
            }
        }
    }
}

/// Glob-match a single path component: `*` matches any run of characters and `?`
/// matches exactly one (neither crosses `/`, since components contain none).
fn segment_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_pi, mut star_ti): (Option<usize>, usize) = (None, 0);

    while ti < txt.len() {
        if pi < pat.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == '*' {
            // Record a backtracking point: `*` matches zero chars for now.
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(sp) = star_pi {
            // Backtrack: let the last `*` consume one more character.
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    // Trailing `*`s match the empty string.
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }
    pi == pat.len()
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

    #[test]
    fn test_matches_pattern_multiple_wildcards() {
        // Patterns with 2+ wildcards previously fell through to exact match and
        // silently matched nothing.
        assert!(matches_pattern("src/a/file.rs", "src/*/*.rs"));
        assert!(matches_pattern(
            "src/components/Button.test.tsx",
            "**/*.test.tsx"
        ));
        assert!(matches_pattern("a/b/c/d.rs", "a/**/d.rs"));
        // `*` must not cross a directory separator.
        assert!(!matches_pattern("src/a/b/file.rs", "src/*/*.rs"));
    }

    #[test]
    fn test_matches_pattern_respects_component_boundaries() {
        // `**/foo.tsx` must match on a component boundary, not a raw suffix.
        assert!(matches_pattern("a/b/foo.tsx", "**/foo.tsx"));
        assert!(matches_pattern("foo.tsx", "**/foo.tsx"));
        assert!(!matches_pattern("barfoo.tsx", "**/foo.tsx"));
        // A literal prefix must align with a component boundary too.
        assert!(matches_pattern("src/x/y.tsx", "src/**/*.tsx"));
        assert!(!matches_pattern("src-gen/y.tsx", "src/**/*.tsx"));
    }
}
