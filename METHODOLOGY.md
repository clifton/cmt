# CMT Methodology: Diff Assembly and LLM Communication

This document describes how `cmt` assembles git diff context and sends it to the LLM for commit message generation.

## Pipeline Overview

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. GIT EXTRACTION (src/git.rs)                                  │
│    ├─ Get staged diff with context lines                       │
│    ├─ Filter: lock files, images, build artifacts              │
│    ├─ Truncate: long lines, large files                        │
│    └─ Collect: per-file statistics                             │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ 2. SEMANTIC ANALYSIS (src/analysis.rs)                          │
│    ├─ Categorize files (source, test, docs, config, ci, build) │
│    ├─ Count: insertions/deletions per category                 │
│    ├─ Suggest: commit type based on patterns                   │
│    └─ Generate: markdown summary for LLM                       │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ 3. PROMPT ASSEMBLY (src/lib.rs + src/prompts/)                  │
│    ├─ Prepend: README excerpt + branch name + recent commits   │
│    ├─ Insert: semantic analysis summary                        │
│    ├─ Append: unified diff (the {{changes}} payload)           │
│    └─ System: prompt + optional user hint                      │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ 4. LLM API CALL (src/ai/mod.rs)                                 │
│    ├─ Check: provider availability + API key                   │
│    ├─ Route: to Claude, OpenAI, or Gemini                      │
│    ├─ Request: structured output via rstructor                 │
│    └─ Receive: JSON-parsed CommitTemplate + token usage        │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ 5. POST-PROCESSING (src/lib.rs + src/templates.rs)              │
│    ├─ Validate: lowercase subject, remove trailing period      │
│    ├─ Clean: scope (lowercase, remove generic values)          │
│    ├─ Deduplicate: remove details that echo subject            │
│    └─ Render: template using Handlebars                        │
└─────────────────────────────────────────────────────────────────┘
```

## 1. Diff Assembly

**Source:** `src/git.rs` - `get_staged_changes()` function

### Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `context_lines` | 20 | Lines of context around changes in unified diff |
| `max_lines_per_file` | 2000 | Maximum diff lines per file before truncation |
| `max_line_width` | 500 | Maximum characters per line before truncation |

### Process

1. **Generate diff** using git2's `diff_tree_to_index()` - compares staged index against HEAD
2. **Collect per-file statistics** using the Patch API (`patch.line_stats()`) for accurate insertion/deletion counts
3. **Apply adaptive trimming** for very large diffs:
   - Trigger: >100 files OR >20,000 total line changes
   - Effect: context_lines clamped to 8-15, max_lines_per_file capped at 500
4. **Format diff text** line-by-line, truncating lines exceeding `max_line_width` with `...`

### Statistics Collected

```rust
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub file_changes: Vec<(String, usize, usize)>,  // (filename, adds, dels)
    pub has_unstaged: bool,
}
```

## 2. File Filtering

**Source:** `src/git.rs` - `is_skippable()` function

Files excluded from the diff sent to the LLM:

### Lock Files
- Extension: `.lock`
- Specific files: `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `cargo.lock`

### Generated Assets
- Extensions: `.map`, `.min.js`, `.min.css`

### Binary/Media
- Extensions: `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.avif`, `.bmp`, `.ico`, `.svg`

### Build Artifacts
- Paths starting with: `dist/`, `build/`

## 3. Semantic Analysis

**Source:** `src/analysis.rs` - `analyze_diff()` function

### File Categorization

| Category | Examples |
|----------|----------|
| Source | `.rs`, `.go`, `.py`, `.js`, `.ts`, `.java`, `.c`, `.cpp`, `.rb`, `.php` |
| Test | `_test.rs`, `.test.js`, `.spec.ts`, `tests/`, `test/` directories |
| Docs | `.md`, `.rst`, `docs/`, `README`, `CHANGELOG` |
| Config | `Cargo.toml`, `package.json`, `.yaml`, `.toml`, `.json` |
| CI | `.github/workflows/`, `.gitlab-ci.yml`, `.travis.yml` |
| Build | `Dockerfile`, `Makefile`, `build.rs`, `CMakeLists.txt` |

### Commit Type Suggestion

Priority-based suggestions generated from file analysis:

| Signal | Type | Condition |
|--------|------|-----------|
| Strong | `docs` | Only documentation files changed |
| Strong | `ci` | Only CI/CD configuration changed |
| Strong | `test` | Only test files changed |
| Strong | `build` | Only build files changed |
| Strong | `chore` | Only config/dependencies changed |
| Strong | `refactor` | Only file renames |
| Weak | `feat` | New source files added |
| Weak | `refactor` | Only deletions |

### Scope Detection

- Only for monorepos with `packages/`, `apps/`, `libs/` directories
- Suggested when >80% of changes are in the same component

## 4. Prompt Construction

**Source:** `src/lib.rs`, `src/prompts/`

### User Prompt Assembly Order

1. **README excerpt** - First 50 lines of project README.md
2. **Branch name** - Current branch (omitted for `main`, `master`, or detached HEAD)
3. **Recent commits** - Last N commits for style context (default: 10, skipped for extremely large diffs)
4. **Pre-analysis summary** - Markdown summary from semantic analysis
5. **Diff text** - The `{{changes}}` payload with full unified diff

### System Prompt

Located at `src/prompts/system_prompt.txt`, defines:
- Commit message format: `{type}: {subject}`
- Type priority order: `fix > feat > perf > refactor > test > build > ci > chore > style > docs`
- Scope rules: only for monorepos
- Length constraints: subject under 50 characters
- Anti-patterns to avoid

### User Hints

Optional `--hint` flag appends additional context to the system prompt.

## Example Prompts

The following examples show fully constructed prompts with default parameters.

### System Prompt

```
Generate concise, professional commit messages from git diffs.
Output plain UTF-8 text only.

Format: {type}: {subject}

SCOPE: Do NOT include scope unless explicitly told to. Scope is only for monorepos with packages/apps/services directories. For single projects, always omit scope.

LENGTH: First line ideally under 50 chars. Details optional.

## Commit Types (highest priority first)

1. fix - Bug fixes
2. feat - New features
3. perf - Performance
4. refactor - Code restructuring
5. test - Tests
6. build - Build/deps
7. ci - CI/CD
8. chore - Maintenance
9. style - Formatting
10. docs - Documentation only

## Examples

```
fix: prevent crash on expired session
```

```
feat: add password reset endpoint
```

```
refactor: extract validation into helper
```

## Anti-Patterns

✗ Using scope for single projects (NO: feat(cli), feat(core), feat(api))
✗ "feat: update code" - too vague
✗ "fix: fix bug" - redundant
✗ Capital first letter in subject
✗ Trailing period in subject

## Rules

- Classify by IMPACT, not volume. One-line fix + docs = "fix"
- Be specific: "fix null pointer in user lookup" not "fix bug"
- If subject fully explains the change, omit bullet points
- Never repeat subject content in bullets
- Focus on WHAT and WHY, not HOW
```

**With `--hint "This fixes the login timeout issue"`:**

```
[system prompt above...]

Additional context: This fixes the login timeout issue
```

### User Prompt (Full Example)

This example shows a complete user prompt with all context sections:

```
Project README:
# cmt - AI-Powered Git Commit Message Generator

`cmt` is a command-line tool that generates meaningful git commit messages using AI models.

## Features

- Supports multiple AI providers (Gemini, Claude, OpenAI)
- Follows conventional commit format
- Rich context: README, branch name, recent commits

Branch: feature/add-auth

Recent commits for context:
[1] feat: add user registration endpoint
[2] fix: resolve session timeout issue
[3] refactor: extract validation helpers
[4] docs: update API documentation
[5] test: add integration tests for auth

# Pre-Analysis of Changes

The following analysis was generated automatically from the diff.
Use this to inform your commit type selection, but always verify by reading the actual diff.

## Change Summary

2 files changed: +45 insertions, -12 deletions

## Files by Category

- source: 2 files (1 added, 1 modified) [+45/-12]

## Changed Files

+ src/auth/login.rs [source]
~ src/auth/mod.rs [source]

## Analysis Hints

WEAK SIGNAL: Consider 'feat' - new source files added
- 1 new source file added

---

Generate a professional commit message for this diff:

```diff
diff --git a/src/auth/login.rs b/src/auth/login.rs
new file mode 100644
index 0000000..a1b2c3d
--- /dev/null
+++ b/src/auth/login.rs
@@ -0,0 +1,35 @@
+use crate::auth::{Session, User};
+use crate::error::AuthError;
+
+/// Handles user login with email and password
+pub async fn login(email: &str, password: &str) -> Result<Session, AuthError> {
+    let user = User::find_by_email(email)
+        .await?
+        .ok_or(AuthError::InvalidCredentials)?;
+
+    if !user.verify_password(password)? {
+        return Err(AuthError::InvalidCredentials);
+    }
+
+    if user.is_locked() {
+        return Err(AuthError::AccountLocked);
+    }
+
+    let session = Session::create(&user).await?;
+    Ok(session)
+}
+
+/// Handles user logout
+pub async fn logout(session: &Session) -> Result<(), AuthError> {
+    session.invalidate().await?;
+    Ok(())
+}
diff --git a/src/auth/mod.rs b/src/auth/mod.rs
index 1234567..89abcde 100644
--- a/src/auth/mod.rs
+++ b/src/auth/mod.rs
@@ -1,5 +1,7 @@
 mod session;
 mod user;
+mod login;

 pub use session::Session;
 pub use user::User;
+pub use login::{login, logout};
```

Instructions:
1. If pre-analysis was provided above, use it to help select the commit type
2. ALWAYS verify by reading the actual diff - the analysis is a hint, not a rule
3. Focus on the PRIMARY purpose of the change (what problem does it solve?)
4. For the subject line: be specific, use present tense, max 50 chars
5. For details: explain WHY, not just WHAT changed

Remember the type priority hierarchy:
fix > feat > perf > refactor > test > build > ci > chore > style > docs

A one-line bug fix with extensive docs = "fix", not "docs"
```

### Minimal User Prompt (No Context)

When README is missing, branch is `main`, and no recent commits:

```
# Pre-Analysis of Changes

The following analysis was generated automatically from the diff.
Use this to inform your commit type selection, but always verify by reading the actual diff.

## Change Summary

1 file changed: +5 insertions, -2 deletions

## Files by Category

- source: 1 file (modified) [+5/-2]

## Changed Files

~ src/main.rs [source]

---

Generate a professional commit message for this diff:

```diff
diff --git a/src/main.rs b/src/main.rs
index abc1234..def5678 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,8 +10,11 @@ fn main() {
     let config = Config::load();
-    let result = process(config);
-    println!("{}", result);
+    let result = match process(config) {
+        Ok(output) => output,
+        Err(e) => {
+            eprintln!("Error: {}", e);
+            std::process::exit(1);
+        }
+    };
+    println!("{}", result);
 }
```

Instructions:
1. If pre-analysis was provided above, use it to help select the commit type
2. ALWAYS verify by reading the actual diff - the analysis is a hint, not a rule
3. Focus on the PRIMARY purpose of the change (what problem does it solve?)
4. For the subject line: be specific, use present tense, max 50 chars
5. For details: explain WHY, not just WHAT changed

Remember the type priority hierarchy:
fix > feat > perf > refactor > test > build > ci > chore > style > docs

A one-line bug fix with extensive docs = "fix", not "docs"
```

### Structured Output JSON Schema

The LLM is constrained to return JSON conforming to this schema (generated by rstructor from `CommitTemplate`):

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CommitTemplate",
  "description": "Commit message data. Format: '{commit_type}: {subject}'. Keep first line under 50 chars. Do NOT use scope.",
  "type": "object",
  "required": ["commit_type", "subject"],
  "properties": {
    "commit_type": {
      "description": "The type of the commit message. Select from CommitType based on the change.",
      "$ref": "#/$defs/CommitType"
    },
    "subject": {
      "type": "string",
      "description": "Brief subject line, ideally under 50 chars total with type prefix. Start with lowercase verb (add, fix, update). Be specific.",
      "example": "add user login endpoint"
    },
    "details": {
      "type": ["string", "null"],
      "description": "Optional details as bullet points (max 79 chars each). Start each bullet with '- ' followed by present tense verb. Focus on explaining the change's purpose and impact.",
      "example": "- Add JWT auth for security\n- Update tests for coverage"
    },
    "issues": {
      "type": ["string", "null"],
      "description": "Optional issue/ticket references. Format: '#123' or 'Fixes #456'",
      "example": "#123"
    },
    "breaking": {
      "type": ["string", "null"],
      "description": "Optional breaking change description. Include when your change breaks backward compatibility.",
      "example": "Drop support for old API"
    },
    "scope": {
      "type": ["string", "null"],
      "description": "LEAVE NULL. Only set for monorepos with packages/apps directories. Do not use for single projects.",
      "example": "auth"
    }
  },
  "$defs": {
    "CommitType": {
      "type": "string",
      "description": "The type of a commit message. Choose based on the PRIMARY purpose using priority: fix > feat > perf > refactor > test > build > ci > chore > style > docs. If a commit fixes a bug AND updates docs, use 'fix'.",
      "enum": ["fix", "feat", "perf", "refactor", "test", "build", "ci", "chore", "style", "docs"],
      "enumDescriptions": {
        "fix": "PRIORITY 1: Bug fix or error correction. Use if ANY bug is fixed, even with other changes.",
        "feat": "PRIORITY 2: New feature or enhancement to functionality (not docs/readme).",
        "perf": "PRIORITY 3: Performance improvements. Use when the primary goal is optimization.",
        "refactor": "PRIORITY 4: Code restructuring WITHOUT behavior change. Only use if no bugs fixed and no features added.",
        "test": "PRIORITY 5: Test additions or updates. Use when changes are primarily about test coverage.",
        "build": "PRIORITY 6: Build system or external dependency changes. E.g., Dockerfile, Makefile.",
        "ci": "PRIORITY 7: CI/CD configuration changes. E.g., GitHub Actions, Jenkins.",
        "chore": "PRIORITY 8: Maintenance tasks, internal dependency updates, tooling.",
        "style": "PRIORITY 9: Formatting or stylistic changes ONLY. No logic changes.",
        "docs": "PRIORITY 10 (LOWEST): Documentation ONLY. Use ONLY when there are NO code logic changes."
      }
    }
  }
}
```

### Example LLM Response

```json
{
  "commit_type": "feat",
  "subject": "add login and logout endpoints",
  "details": "- Implement JWT-based session management\n- Add password verification with proper error handling\n- Support account lockout detection",
  "issues": null,
  "breaking": null,
  "scope": null
}
```

## 5. LLM Communication

**Source:** `src/ai/mod.rs`

### Supported Providers

| Provider | Default Model | API Key Env Var |
|----------|---------------|-----------------|
| Gemini | `gemini-3-flash-preview` | `GEMINI_API_KEY` |
| Claude | `claude-sonnet-4-5-20250929` | `ANTHROPIC_API_KEY` |
| OpenAI | `gpt-5.2` | `OPENAI_API_KEY` |

### Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `temperature` | 0.3 | Creativity level (0.0-2.0) |
| `thinking` | `low` | Reasoning depth: none, minimal, low, high |

### Structured Output

Uses [rstructor](https://github.com/clifton/rstructor) for type-safe structured output:

```rust
pub struct CommitTemplate {
    pub commit_type: CommitType,  // Fix, Feat, Perf, Refactor, Test, Build, Ci, Chore, Style, Docs
    pub subject: String,          // max 50 chars, lowercase, no period
    pub details: Option<String>,  // bullet points
    pub issues: Option<String>,   // "#123" or "Fixes #456"
    pub breaking: Option<String>, // breaking change description
    pub scope: Option<String>,    // only for monorepos
}
```

## 6. Post-Processing

**Source:** `src/lib.rs` - `validate_commit_data()`, `src/templates.rs`

### Validation Rules

1. **Subject normalization** - Force lowercase first character, remove trailing period
2. **Scope cleanup** - Lowercase, replace spaces with hyphens, remove generic values (`general`, `misc`, `other`, `null`)
3. **Detail deduplication** - Remove bullet points that duplicate the subject

### Template Rendering

Uses Handlebars templates. Built-in templates:

| Template | Format |
|----------|--------|
| `simple` | `{subject}\n\n{details}` |
| `conventional` | `{type}({scope}): {subject}\n\n{details}` |
| `detailed` | Adds `Fixes:` and `BREAKING CHANGE:` sections |

Custom templates stored in `~/.config/cmt/templates/*.hbs`

## Default Parameters Summary

| Parameter | Default | Source |
|-----------|---------|--------|
| `context_lines` | 20 | `src/config/defaults.rs` |
| `max_lines_per_file` | 2000 | `src/config/defaults.rs` |
| `max_line_width` | 500 | `src/config/defaults.rs` |
| `temperature` | 0.3 | `src/ai/mod.rs` |
| `thinking` | `low` | `src/config/cli.rs` |
| `provider` | `gemini` | `src/config/defaults.rs` |
| `template` | `conventional` | `src/config/defaults.rs` |
| `recent_commits_count` | 10 | `src/config/defaults.rs` |
| `include_recent_commits` | true | `src/config/defaults.rs` |

## Adaptive Behavior

### Large Diff Handling

| Threshold | Action |
|-----------|--------|
| >100 files OR >20k changes | Reduce context to 8-15 lines, cap 500 lines/file |
| >150 files OR >50k changes | Skip recent commits context entirely |

### Token Budget

Defaults are tuned for Gemini Flash's 1M token context window while maintaining reasonable context for smaller models.
