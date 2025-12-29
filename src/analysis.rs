//! Diff analysis module for extracting semantic information from git diffs.
//!
//! This module provides structured analysis of git diffs to help the AI
//! make better commit type classifications.

use git2::{Delta, DiffOptions, Repository};
use std::collections::HashMap;
use std::path::Path;

/// Categories of files based on their path and purpose
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileCategory {
    /// Source code files (excluding tests)
    Source,
    /// Test files
    Test,
    /// Documentation files (README, docs/, *.md)
    Docs,
    /// Configuration files (.toml, .json, .yaml, etc.)
    Config,
    /// CI/CD files (.github/, .gitlab-ci.yml, etc.)
    Ci,
    /// Build system files (Makefile, Dockerfile, build scripts)
    Build,
    /// Other/unknown files
    Other,
}

impl FileCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileCategory::Source => "source",
            FileCategory::Test => "test",
            FileCategory::Docs => "docs",
            FileCategory::Config => "config",
            FileCategory::Ci => "ci",
            FileCategory::Build => "build",
            FileCategory::Other => "other",
        }
    }
}

/// Type of file operation in the diff
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

impl FileOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileOperation::Added => "added",
            FileOperation::Modified => "modified",
            FileOperation::Deleted => "deleted",
            FileOperation::Renamed => "renamed",
            FileOperation::Copied => "copied",
        }
    }
}

/// Information about a single changed file
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub old_path: Option<String>, // For renames
    pub operation: FileOperation,
    pub category: FileCategory,
    pub insertions: usize,
    pub deletions: usize,
}

/// Statistics for a file category
#[derive(Debug, Clone, Default)]
pub struct CategoryStats {
    pub files: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
}

/// Suggested commit type based on diff analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestedType {
    /// Strong signal for this type
    Strong(&'static str),
    /// Weak signal, could be this type
    Weak(&'static str),
    /// No clear signal
    Unknown,
}

/// Complete analysis of a diff
#[derive(Debug, Clone)]
pub struct DiffAnalysis {
    pub files: Vec<FileChange>,
    pub category_stats: HashMap<FileCategory, CategoryStats>,
    pub total_insertions: usize,
    pub total_deletions: usize,
    pub total_files: usize,
    pub suggested_type: SuggestedType,
    pub confidence_reasons: Vec<String>,
}

impl DiffAnalysis {
    /// Suggest a scope based on common directory or component.
    /// Only suggests scope for clearly structured projects (monorepos, large codebases).
    pub fn suggest_scope(&self) -> Option<String> {
        if self.files.is_empty() {
            return None;
        }

        // Well-known scope patterns that are meaningful
        let valid_scopes = [
            "frontend",
            "backend",
            "api",
            "web",
            "mobile",
            "ios",
            "android",
            "cli",
            "core",
            "common",
            "shared",
            "server",
            "client",
            "ui",
            "auth",
            "db",
            "database",
            "infra",
            "deploy",
            "docs",
            "test",
            "tests",
        ];

        // Monorepo patterns that indicate scope is appropriate
        let monorepo_roots = ["packages", "apps", "libs", "services", "modules", "crates"];

        // Extract meaningful directory components
        let components: Vec<Option<String>> = self
            .files
            .iter()
            .filter_map(|f| {
                let path = Path::new(&f.path);
                let parts: Vec<_> = path.components().collect();

                // Skip if it's just a file in root or shallow (< 3 levels suggests small project)
                if parts.len() < 3 {
                    return None;
                }

                let mut iter = parts.iter().filter_map(|c| {
                    if let std::path::Component::Normal(s) = c {
                        Some(s.to_string_lossy().to_lowercase())
                    } else {
                        None
                    }
                });

                let first = iter.next()?;

                // If it's a monorepo root, get the package name
                if monorepo_roots.contains(&first.as_str()) {
                    return iter.next();
                }

                // Skip generic directories, look for meaningful scope
                let skip_prefixes = ["src", "lib", "app", "pkg", "internal", "cmd"];
                if skip_prefixes.contains(&first.as_str()) {
                    iter.next()
                } else if valid_scopes.contains(&first.as_str()) {
                    Some(first)
                } else {
                    None // Don't suggest arbitrary directory names
                }
            })
            .map(Some)
            .collect();

        // Find the most common component
        let mut counts: HashMap<String, usize> = HashMap::new();
        for comp in components.into_iter().flatten() {
            *counts.entry(comp).or_insert(0) += 1;
        }

        // Only suggest if one component covers >80% of files (very clear scope)
        // and we have multiple files (not just one file in a subdir)
        let total = self.files.len();
        if total < 2 {
            return None;
        }

        counts
            .into_iter()
            .filter(|(name, count)| {
                *count as f64 / total as f64 > 0.8 // Must be very dominant
                    && !name.is_empty()
                    && name.len() <= 15
                    && !name.contains('.')
            })
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name)
    }

    /// Generate a summary string for the AI prompt
    pub fn summary(&self) -> String {
        let mut summary = String::new();

        // Overall stats
        summary.push_str(&format!(
            "## Change Summary\n{} files changed: +{} insertions, -{} deletions\n",
            self.total_files, self.total_insertions, self.total_deletions
        ));

        summary.push('\n');

        // Category breakdown
        summary.push_str("## Files by Category\n");
        for (category, stats) in &self.category_stats {
            if stats.files > 0 {
                let mut ops = Vec::new();
                if stats.added > 0 {
                    ops.push(format!("{} added", stats.added));
                }
                if stats.modified > 0 {
                    ops.push(format!("{} modified", stats.modified));
                }
                if stats.deleted > 0 {
                    ops.push(format!("{} deleted", stats.deleted));
                }
                if stats.renamed > 0 {
                    ops.push(format!("{} renamed", stats.renamed));
                }

                summary.push_str(&format!(
                    "- {}: {} files ({}) [+{}/-{}]\n",
                    category.as_str(),
                    stats.files,
                    ops.join(", "),
                    stats.insertions,
                    stats.deletions
                ));
            }
        }

        // File list
        summary.push_str("\n## Changed Files (top 20 by churn)\n");
        let mut files = self.files.clone();
        files.sort_by_key(|f| -(f.insertions as isize + f.deletions as isize));
        let top_n = 20usize.min(files.len());
        for file in files.iter().take(top_n) {
            let op_indicator = match file.operation {
                FileOperation::Added => "+",
                FileOperation::Deleted => "-",
                FileOperation::Modified => "~",
                FileOperation::Renamed => "→",
                FileOperation::Copied => "c",
            };
            if let Some(old_path) = &file.old_path {
                summary.push_str(&format!(
                    "{} {} → {} [{}]\n",
                    op_indicator,
                    old_path,
                    file.path,
                    file.category.as_str()
                ));
            } else {
                summary.push_str(&format!(
                    "{} {} [{}]\n",
                    op_indicator,
                    file.path,
                    file.category.as_str()
                ));
            }
        }
        if self.files.len() > top_n {
            summary.push_str(&format!(
                "+{} other files not listed\n",
                self.files.len() - top_n
            ));
        }

        // Suggested type
        summary.push_str("\n## Analysis Hints\n");
        match self.suggested_type {
            SuggestedType::Strong(t) => {
                summary.push_str(&format!(
                    "STRONG SIGNAL: This appears to be a '{}' commit\n",
                    t
                ));
            }
            SuggestedType::Weak(t) => {
                summary.push_str(&format!("POSSIBLE: This might be a '{}' commit\n", t));
            }
            SuggestedType::Unknown => {
                summary.push_str("No clear pattern detected - analyze the diff carefully\n");
            }
        }

        for reason in &self.confidence_reasons {
            summary.push_str(&format!("- {}\n", reason));
        }

        summary
    }
}

/// Determine the category of a file based on its path
fn categorize_file(path: &Path) -> FileCategory {
    let path_str = path.to_string_lossy().to_lowercase();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // CI/CD detection
    if path_str.starts_with(".github/")
        || path_str.starts_with(".gitlab")
        || path_str.starts_with(".circleci/")
        || path_str.starts_with(".travis")
        || file_name == ".travis.yml"
        || file_name == "azure-pipelines.yml"
        || file_name == "jenkinsfile"
        || path_str.contains("ci/")
        || path_str.contains("ci-")
    {
        return FileCategory::Ci;
    }

    // Test detection
    if path_str.contains("/tests/")
        || path_str.contains("/test/")
        || path_str.contains("_test.")
        || path_str.contains(".test.")
        || path_str.contains("_spec.")
        || path_str.contains(".spec.")
        || path_str.starts_with("tests/")
        || path_str.starts_with("test/")
        || file_name.starts_with("test_")
        || file_name.ends_with("_test.rs")
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.py")
        || file_name.ends_with(".test.js")
        || file_name.ends_with(".test.ts")
        || file_name.ends_with(".spec.js")
        || file_name.ends_with(".spec.ts")
    {
        return FileCategory::Test;
    }

    // Documentation detection
    if path_str.starts_with("docs/")
        || path_str.starts_with("doc/")
        || path_str.contains("/docs/")
        || path_str.contains("/doc/")
        || file_name == "readme.md"
        || file_name == "readme.rst"
        || file_name == "readme.txt"
        || file_name == "readme"
        || file_name == "changelog.md"
        || file_name == "changelog"
        || file_name == "history.md"
        || file_name == "contributing.md"
        || file_name == "license"
        || file_name == "license.md"
        || file_name == "license.txt"
        || extension == "md"
        || extension == "rst"
        || extension == "txt"
    {
        return FileCategory::Docs;
    }

    // Build system detection
    if file_name == "dockerfile"
        || file_name == "makefile"
        || file_name == "cmakelists.txt"
        || file_name == "build.gradle"
        || file_name == "build.gradle.kts"
        || file_name == "pom.xml"
        || file_name == "build.rs"
        || file_name == "build.zig"
        || file_name.ends_with(".dockerfile")
        || path_str.contains("docker-compose")
    {
        return FileCategory::Build;
    }

    // Config detection
    if file_name == "cargo.toml"
        || file_name == "package.json"
        || file_name == "package-lock.json"
        || file_name == "yarn.lock"
        || file_name == "pyproject.toml"
        || file_name == "setup.py"
        || file_name == "setup.cfg"
        || file_name == "requirements.txt"
        || file_name == "go.mod"
        || file_name == "go.sum"
        || file_name == "tsconfig.json"
        || file_name == "eslintrc.json"
        || file_name == ".eslintrc"
        || file_name == ".prettierrc"
        || file_name == "rustfmt.toml"
        || file_name == ".rustfmt.toml"
        || file_name == "clippy.toml"
        || file_name == ".clippy.toml"
        || file_name.starts_with(".")
        || extension == "toml"
        || extension == "yaml"
        || extension == "yml"
        || extension == "json"
        || extension == "ini"
        || extension == "cfg"
    {
        return FileCategory::Config;
    }

    // Source code detection (common extensions)
    let source_extensions = [
        "rs", "go", "py", "js", "ts", "tsx", "jsx", "java", "kt", "scala", "c", "cpp", "cc", "cxx",
        "h", "hpp", "cs", "rb", "php", "swift", "m", "mm", "zig", "nim", "lua", "r", "sql", "sh",
        "bash", "zsh", "fish", "ps1", "pl", "pm", "ex", "exs", "erl", "hrl", "hs", "ml", "mli",
        "fs", "fsi", "fsx", "clj", "cljs", "cljc", "elm", "vue", "svelte",
    ];

    if source_extensions.contains(&extension.as_str()) {
        return FileCategory::Source;
    }

    FileCategory::Other
}

/// Analyze a git diff and return structured information
pub fn analyze_diff(repo: &Repository) -> Result<DiffAnalysis, git2::Error> {
    let mut opts = DiffOptions::new();
    opts.include_untracked(false);

    // Enable rename detection
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    find_opts.copies(true);

    let tree = repo.head().and_then(|head| head.peel_to_tree()).ok(); // New repo with no commits

    let mut diff = repo.diff_tree_to_index(tree.as_ref(), None, Some(&mut opts))?;

    // Run rename/copy detection
    diff.find_similar(Some(&mut find_opts))?;

    let mut files = Vec::new();
    let mut category_stats: HashMap<FileCategory, CategoryStats> = HashMap::new();
    let mut total_insertions = 0;
    let mut total_deletions = 0;

    // Iterate through diff deltas
    for delta_idx in 0..diff.deltas().len() {
        let delta = diff.get_delta(delta_idx).unwrap();

        let new_file = delta.new_file();
        let old_file = delta.old_file();

        let path = new_file
            .path()
            .or_else(|| old_file.path())
            .unwrap_or(Path::new(""));

        // Skip lock files
        if path.extension().is_some_and(|ext| ext == "lock") {
            continue;
        }

        let operation = match delta.status() {
            Delta::Added => FileOperation::Added,
            Delta::Deleted => FileOperation::Deleted,
            Delta::Modified => FileOperation::Modified,
            Delta::Renamed => FileOperation::Renamed,
            Delta::Copied => FileOperation::Copied,
            _ => FileOperation::Modified,
        };

        let old_path = if operation == FileOperation::Renamed || operation == FileOperation::Copied
        {
            old_file.path().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        let category = categorize_file(path);

        // Get stats for this file
        let (insertions, deletions) = get_file_stats(&diff, delta_idx);
        total_insertions += insertions;
        total_deletions += deletions;

        let file_change = FileChange {
            path: path.to_string_lossy().to_string(),
            old_path,
            operation,
            category,
            insertions,
            deletions,
        };

        // Update category stats
        let stats = category_stats.entry(category).or_default();
        stats.files += 1;
        stats.insertions += insertions;
        stats.deletions += deletions;
        match operation {
            FileOperation::Added => stats.added += 1,
            FileOperation::Modified => stats.modified += 1,
            FileOperation::Deleted => stats.deleted += 1,
            FileOperation::Renamed => stats.renamed += 1,
            FileOperation::Copied => {}
        }

        files.push(file_change);
    }

    let total_files = files.len();

    // Determine suggested type based on patterns
    let (suggested_type, confidence_reasons) =
        suggest_commit_type(&files, &category_stats, total_insertions, total_deletions);

    Ok(DiffAnalysis {
        files,
        category_stats,
        total_insertions,
        total_deletions,
        total_files,
        suggested_type,
        confidence_reasons,
    })
}

/// Get insertion/deletion counts for a specific file in the diff
fn get_file_stats(diff: &git2::Diff, delta_idx: usize) -> (usize, usize) {
    let mut insertions = 0;
    let mut deletions = 0;

    let _ = diff.foreach(
        &mut |d, _| d.nfiles() as usize == delta_idx + 1,
        None,
        None,
        Some(&mut |_delta, _hunk, line| {
            match line.origin() {
                '+' => insertions += 1,
                '-' => deletions += 1,
                _ => {}
            }
            true
        }),
    );

    // Fallback: use overall stats if per-file fails
    if insertions == 0 && deletions == 0 {
        if let Ok(stats) = diff.stats() {
            insertions = stats.insertions() / diff.deltas().len().max(1);
            deletions = stats.deletions() / diff.deltas().len().max(1);
        }
    }

    (insertions, deletions)
}

/// Suggest a commit type based on the analysis
fn suggest_commit_type(
    files: &[FileChange],
    category_stats: &HashMap<FileCategory, CategoryStats>,
    _total_insertions: usize,
    _total_deletions: usize,
) -> (SuggestedType, Vec<String>) {
    let mut reasons = Vec::new();

    // Check for pure documentation changes
    let docs_stats = category_stats.get(&FileCategory::Docs);
    let total_non_docs: usize = category_stats
        .iter()
        .filter(|(k, _)| **k != FileCategory::Docs)
        .map(|(_, v)| v.files)
        .sum();

    if docs_stats.is_some_and(|s| s.files > 0) && total_non_docs == 0 {
        reasons.push("All changes are in documentation files".to_string());
        return (SuggestedType::Strong("docs"), reasons);
    }

    // Check for pure CI changes
    let ci_stats = category_stats.get(&FileCategory::Ci);
    let total_non_ci: usize = category_stats
        .iter()
        .filter(|(k, _)| **k != FileCategory::Ci)
        .map(|(_, v)| v.files)
        .sum();

    if ci_stats.is_some_and(|s| s.files > 0) && total_non_ci == 0 {
        reasons.push("All changes are in CI/CD configuration".to_string());
        return (SuggestedType::Strong("ci"), reasons);
    }

    // Check for pure test changes
    let test_stats = category_stats.get(&FileCategory::Test);
    let total_non_test: usize = category_stats
        .iter()
        .filter(|(k, _)| **k != FileCategory::Test)
        .map(|(_, v)| v.files)
        .sum();

    if test_stats.is_some_and(|s| s.files > 0) && total_non_test == 0 {
        reasons.push("All changes are in test files".to_string());
        return (SuggestedType::Strong("test"), reasons);
    }

    // Check for pure build changes
    let build_stats = category_stats.get(&FileCategory::Build);
    let total_non_build: usize = category_stats
        .iter()
        .filter(|(k, _)| **k != FileCategory::Build)
        .map(|(_, v)| v.files)
        .sum();

    if build_stats.is_some_and(|s| s.files > 0) && total_non_build == 0 {
        reasons.push("All changes are in build configuration".to_string());
        return (SuggestedType::Strong("build"), reasons);
    }

    // Check for pure config/dependency changes
    let config_stats = category_stats.get(&FileCategory::Config);
    let total_non_config: usize = category_stats
        .iter()
        .filter(|(k, _)| **k != FileCategory::Config)
        .map(|(_, v)| v.files)
        .sum();

    if config_stats.is_some_and(|s| s.files > 0) && total_non_config == 0 {
        reasons.push("All changes are in configuration/dependency files".to_string());
        return (SuggestedType::Strong("chore"), reasons);
    }

    // Check for renames (suggests refactor)
    let total_renames: usize = category_stats.values().map(|s| s.renamed).sum();
    if total_renames > 0 {
        reasons.push(format!("{} files were renamed", total_renames));
        if total_renames == files.len() {
            return (SuggestedType::Strong("refactor"), reasons);
        } else {
            reasons.push("Renames mixed with other changes".to_string());
        }
    }

    // Check for new files (suggests feat)
    let total_added: usize = category_stats.values().map(|s| s.added).sum();
    let source_stats = category_stats.get(&FileCategory::Source);

    if total_added > 0 && source_stats.is_some_and(|s| s.added > 0) {
        reasons.push(format!(
            "{} new source files added",
            source_stats.map_or(0, |s| s.added)
        ));
        return (SuggestedType::Weak("feat"), reasons);
    }

    // Check for deletions only (could be refactor or chore)
    let total_deleted: usize = category_stats.values().map(|s| s.deleted).sum();
    let total_modified: usize = category_stats.values().map(|s| s.modified).sum();

    if total_deleted > 0 && total_added == 0 && total_modified == 0 {
        reasons.push("Only file deletions, no additions or modifications".to_string());
        return (SuggestedType::Weak("refactor"), reasons);
    }

    // Source code changes without clear signals
    if source_stats.is_some_and(|s| s.files > 0) {
        reasons.push("Source code modified - analyze diff for fix/feat/refactor".to_string());
        return (SuggestedType::Unknown, reasons);
    }

    (SuggestedType::Unknown, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();

        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        (temp_dir, repo)
    }

    fn create_and_stage_file(repo: &Repository, name: &str, content: &str) {
        let path = repo.workdir().unwrap().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut file = File::create(path).unwrap();
        writeln!(file, "{}", content).unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new(name)).unwrap();
        index.write().unwrap();
    }

    #[test]
    fn test_categorize_source_files() {
        assert_eq!(
            categorize_file(Path::new("src/main.rs")),
            FileCategory::Source
        );
        assert_eq!(
            categorize_file(Path::new("lib/utils.py")),
            FileCategory::Source
        );
        assert_eq!(categorize_file(Path::new("app.js")), FileCategory::Source);
    }

    #[test]
    fn test_categorize_test_files() {
        assert_eq!(
            categorize_file(Path::new("tests/test_main.rs")),
            FileCategory::Test
        );
        assert_eq!(
            categorize_file(Path::new("src/utils_test.go")),
            FileCategory::Test
        );
        assert_eq!(
            categorize_file(Path::new("app.test.js")),
            FileCategory::Test
        );
        assert_eq!(
            categorize_file(Path::new("app.spec.ts")),
            FileCategory::Test
        );
    }

    #[test]
    fn test_categorize_docs_files() {
        assert_eq!(categorize_file(Path::new("README.md")), FileCategory::Docs);
        assert_eq!(
            categorize_file(Path::new("docs/guide.md")),
            FileCategory::Docs
        );
        assert_eq!(
            categorize_file(Path::new("CHANGELOG.md")),
            FileCategory::Docs
        );
    }

    #[test]
    fn test_categorize_ci_files() {
        assert_eq!(
            categorize_file(Path::new(".github/workflows/ci.yml")),
            FileCategory::Ci
        );
        assert_eq!(
            categorize_file(Path::new(".gitlab-ci.yml")),
            FileCategory::Ci
        );
        assert_eq!(categorize_file(Path::new(".travis.yml")), FileCategory::Ci);
    }

    #[test]
    fn test_categorize_config_files() {
        assert_eq!(
            categorize_file(Path::new("Cargo.toml")),
            FileCategory::Config
        );
        assert_eq!(
            categorize_file(Path::new("package.json")),
            FileCategory::Config
        );
        assert_eq!(
            categorize_file(Path::new(".eslintrc")),
            FileCategory::Config
        );
    }

    #[test]
    fn test_categorize_build_files() {
        assert_eq!(
            categorize_file(Path::new("Dockerfile")),
            FileCategory::Build
        );
        assert_eq!(categorize_file(Path::new("Makefile")), FileCategory::Build);
        assert_eq!(categorize_file(Path::new("build.rs")), FileCategory::Build);
    }

    #[test]
    fn test_analyze_docs_only_change() {
        let (_temp_dir, repo) = setup_test_repo();
        create_and_stage_file(&repo, "README.md", "# Hello World");

        let analysis = analyze_diff(&repo).unwrap();

        assert_eq!(analysis.total_files, 1);
        assert!(matches!(
            analysis.suggested_type,
            SuggestedType::Strong("docs")
        ));
    }

    #[test]
    fn test_analyze_source_change() {
        let (_temp_dir, repo) = setup_test_repo();
        create_and_stage_file(&repo, "src/main.rs", "fn main() {}");

        let analysis = analyze_diff(&repo).unwrap();

        assert_eq!(analysis.total_files, 1);
        assert!(analysis.category_stats.contains_key(&FileCategory::Source));
    }

    #[test]
    fn test_analyze_mixed_changes() {
        let (_temp_dir, repo) = setup_test_repo();
        create_and_stage_file(&repo, "src/main.rs", "fn main() {}");
        create_and_stage_file(&repo, "README.md", "# Hello");

        let analysis = analyze_diff(&repo).unwrap();

        assert_eq!(analysis.total_files, 2);
        assert!(analysis.category_stats.contains_key(&FileCategory::Source));
        assert!(analysis.category_stats.contains_key(&FileCategory::Docs));
    }

    #[test]
    fn test_summary_generation() {
        let (_temp_dir, repo) = setup_test_repo();
        create_and_stage_file(&repo, "src/main.rs", "fn main() {}");

        let analysis = analyze_diff(&repo).unwrap();
        let summary = analysis.summary();

        assert!(summary.contains("Change Summary"));
        assert!(summary.contains("Files by Category"));
        assert!(summary.contains("Changed Files"));
        assert!(summary.contains("Analysis Hints"));
    }
}
