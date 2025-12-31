# CMT - Claude Code Instructions

## Project Overview

`cmt` is a Rust CLI tool that generates git commit messages using LLMs (Claude, OpenAI, Gemini). It analyzes staged changes, builds rich context, and produces conventional commit messages via structured output.

## Key Files

- `src/git.rs` - Git diff extraction, filtering, and statistics
- `src/analysis.rs` - Semantic analysis and commit type suggestion
- `src/lib.rs` - Main generation logic and post-processing
- `src/ai/mod.rs` - LLM provider abstraction
- `src/prompts/` - System and user prompt templates
- `src/templates.rs` - Handlebars template rendering
- `src/config/` - CLI args, config file loading, defaults
- `src/bin/main.rs` - CLI entry point

## Implementation Details

See **METHODOLOGY.md** for detailed documentation of:
- How git diffs are assembled and filtered
- Semantic analysis and commit type suggestion
- Prompt construction and LLM communication
- All default parameters and adaptive behavior

**When modifying the diff-to-LLM pipeline, update METHODOLOGY.md to keep it accurate.**

## Build & Test

```bash
cargo build --release    # Build
cargo test               # Run tests
cargo clippy             # Lint
```

## Dependencies

- `git2` - Git operations
- `rstructor` - Structured LLM output
- `handlebars` - Template rendering
- `tokio` - Async runtime
