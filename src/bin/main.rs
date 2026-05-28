use arboard::Clipboard;
use cmt::ai_mod::{default_model, list_models};
use cmt::config_mod::{file as config_file, Config};
use cmt::pricing::{self, PricingCache};
use cmt::template_mod::TemplateManager;
use cmt::{
    append_to_cmtignore, create_commit, generate_commit_message, get_current_branch,
    get_readme_excerpt, load_cmtignore, Args, CommitError, CommitOptions, Spinner,
};
use colored::*;
use dotenv::dotenv;
use git2::Repository;
use std::io::{self, IsTerminal, Write};
use std::time::Instant;
use std::{env, process};

enum CommitAction {
    Commit,
    Cancel,
    Hint,
    Edit,
}

/// Open `message` in the user's editor and return the edited text.
///
/// Resolves the editor like git does (GIT_EDITOR -> core.editor -> VISUAL ->
/// EDITOR -> vi), strips comment lines, and returns None if the editor failed
/// or the message was emptied (in which case the previous message is kept).
fn edit_in_editor(message: &str) -> Option<String> {
    use std::process::Command;

    let editor = std::env::var("GIT_EDITOR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["config", "--get", "core.editor"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            std::env::var("VISUAL")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| "vi".to_string());

    let mut tmp = tempfile::Builder::new()
        .prefix("CMT_EDITMSG_")
        .suffix(".txt")
        .tempfile()
        .ok()?;
    write!(tmp, "{}", message).ok()?;
    let path = tmp.into_temp_path();

    // The editor string may carry args (e.g. "code --wait"); split it.
    let mut parts = editor.split_whitespace();
    let program = parts.next()?;
    let mut cmd = Command::new(program);
    for arg in parts {
        cmd.arg(arg);
    }
    cmd.arg(&path);
    let status = cmd.status().ok()?;
    if !status.success() {
        return None;
    }

    let edited = std::fs::read_to_string(&path).ok()?;
    clean_edited_message(&edited)
}

/// Strip comment lines (`#…`) and surrounding whitespace from an edited message.
/// Returns None if nothing meaningful remains.
fn clean_edited_message(edited: &str) -> Option<String> {
    let cleaned = edited
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_edited_message_strips_comments_and_trims() {
        let edited = "# Please edit your message\nfix: handle empty input\n\n# trailing comment\n";
        assert_eq!(
            clean_edited_message(edited).as_deref(),
            Some("fix: handle empty input")
        );
    }

    #[test]
    fn test_clean_edited_message_empty_returns_none() {
        assert_eq!(clean_edited_message("# only a comment\n\n   \n"), None);
        assert_eq!(clean_edited_message(""), None);
    }

    #[test]
    fn test_clean_edited_message_keeps_body() {
        let edited = "feat: add thing\n\n- detail one\n- detail two\n";
        assert_eq!(
            clean_edited_message(edited).as_deref(),
            Some("feat: add thing\n\n- detail one\n- detail two")
        );
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok(); // Load .env file if it exists
    let args = Args::new_from(env::args());

    // Start pricing fetch in background (will be ready by time generation completes)
    let mut pricing_cache = PricingCache::new();

    // Handle configuration initialization
    if args.init_config {
        match config_file::create_config_file(args.config_path.as_deref()) {
            Ok(path) => {
                println!("{}", "Configuration file created:".green().bold());
                println!("{}", path.display());
                process::exit(0);
            }
            Err(e) => {
                eprintln!("{}", "Error creating configuration file:".red().bold());
                eprintln!("{}", e);
                process::exit(1);
            }
        }
    }

    // Handle listing available models (doesn't need templates)
    if args.list_models {
        let provider_name = &args.provider;

        match list_models(provider_name).await {
            Ok(models) => {
                println!(
                    "{}",
                    format!("Available models for {}:", provider_name)
                        .green()
                        .bold(),
                );

                // Sort models alphabetically for better readability
                let mut sorted_models = models;
                sorted_models.sort();

                let default = default_model(provider_name);
                for model in sorted_models {
                    // Highlight the default model
                    if model == default {
                        println!("- {} (default)", model.cyan());
                    } else {
                        println!("- {}", model);
                    }
                }
                process::exit(0);
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("Error fetching models for {}:", provider_name)
                        .red()
                        .bold()
                );
                eprintln!("{}", e);
                process::exit(1);
            }
        }
    }

    // Handle showing template content (doesn't need TemplateManager)
    if let Some(template_name) = &args.show_template {
        match config_file::get_template(template_name) {
            Ok(content) => {
                println!(
                    "{}",
                    format!("Template '{}':", template_name).green().bold()
                );
                println!("{}", content);
                process::exit(0);
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("Error showing template '{}':", template_name)
                        .red()
                        .bold()
                );
                eprintln!("{}", e);
                process::exit(1);
            }
        }
    }

    // Handle creating a new template (doesn't need TemplateManager)
    if let Some(template_name) = &args.create_template {
        // Ensure template directory exists
        if let Err(e) = config_file::create_template_dir() {
            eprintln!("{}", "Error creating template directory:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }

        // Get template content
        let content = match &args.template_content {
            Some(content) => content.clone(),
            None => {
                eprintln!(
                    "{}",
                    "Error: --template-content is required when creating a template"
                        .red()
                        .bold()
                );
                eprintln!("Example: cmt --create-template my-template --template-content \"{{type}}: {{subject}}\\n\\n{{details}}\"");
                process::exit(1);
            }
        };

        // Save the template
        match config_file::save_template(template_name, &content) {
            Ok(_) => {
                println!(
                    "{}",
                    format!("Template '{}' created successfully.", template_name)
                        .green()
                        .bold()
                );
                println!("You can use it with: cmt --template {}", template_name);
                process::exit(0);
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("Error creating template '{}':", template_name)
                        .red()
                        .bold()
                );
                eprintln!("{}", e);
                process::exit(1);
            }
        }
    }

    // Initialize template manager (only needed for --list-templates and commit generation)
    let template_manager = match TemplateManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            eprintln!("{}", "Error initializing templates:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // Handle listing available templates
    if args.list_templates {
        println!("{}", "Available templates:".green().bold());
        for template in template_manager.list_templates() {
            println!("- {}", template);
        }
        process::exit(0);
    }

    // Load configuration
    let mut config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "{}",
                "Warning: Failed to load configuration:".yellow().bold()
            );
            eprintln!("{}", e);
            Config::default()
        }
    };

    // Override config with CLI args
    let cli_config = Config::from_args(&args);
    config.merge(&cli_config);

    // Only prompt / animate when both stdin and stdout are real terminals. When
    // piped or run in CI, cmt must not block on a closed stdin or read EOF and
    // silently cancel; it relies on flags (-y / --no-commit / -m) instead.
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();

    // Open git repository (discover searches up the directory tree)
    let repo = match Repository::discover(".") {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("{}", "Error opening git repository:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // Get repository root for .cmtignore
    let repo_root = repo.workdir().unwrap_or_else(|| std::path::Path::new("."));

    // Load .cmtignore patterns
    let cmtignore_patterns = load_cmtignore(repo_root);

    // Get staged changes (includes both diff text and stats in one pass).
    // Read from the resolved `config` (defaults < global < project < CLI), not
    // raw `args`, so .cmt.toml settings actually take effect.
    let get_staged = || {
        cmt::get_staged_changes(
            &repo,
            config.context_lines,
            config.max_lines_per_file,
            config.max_line_width,
            config.max_file_lines,
            &cmtignore_patterns,
        )
    };

    let staged = match get_staged() {
        Ok(changes) => changes,
        Err(e) if e.to_string().contains("No changes have been staged") => {
            // Nothing staged — offer to stage tracked changes instead of
            // dead-ending (the most common first-run frustration).
            let unstaged = cmt::has_unstaged_changes(&repo);
            let do_stage = if args.all {
                unstaged
            } else if unstaged && interactive && !args.yes {
                print!(
                    "{}",
                    "Nothing staged. Stage all tracked changes and continue? [y/N] ".cyan()
                );
                let _ = io::stdout().flush();
                let mut input = String::new();
                io::stdin().read_line(&mut input).is_ok()
                    && matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
            } else {
                false
            };

            if do_stage {
                if let Err(err) = cmt::stage_tracked_changes(&repo) {
                    eprintln!("{}", "Error staging changes:".red().bold());
                    eprintln!("{}", err);
                    process::exit(1);
                }
                match get_staged() {
                    Ok(changes) => changes,
                    Err(err) => {
                        eprintln!("{}", "Error:".red().bold());
                        eprintln!("{}", err);
                        process::exit(1);
                    }
                }
            } else {
                eprintln!("{}", "No changes have been staged for commit.".red().bold());
                if unstaged {
                    eprintln!(
                        "{}",
                        "You have unstaged changes — stage them, or re-run with -a/--all.".yellow()
                    );
                }
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{}", "Error:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // Handle files that exceed the threshold (prompt to add to .cmtignore).
    // Only when interactive — never block a piped/CI run on this prompt.
    if !staged.stats.skipped_files.is_empty() && interactive && !args.yes && !config.message_only {
        println!();
        println!(
            "{}",
            format!(
                "The following files exceed {} lines changed:",
                config.max_file_lines
            )
            .yellow()
            .bold()
        );
        for (file, adds, dels) in &staged.stats.skipped_files {
            let total = adds + dels;
            let lines_display = if total >= 1000 {
                format!("{}K lines", total / 1000)
            } else {
                format!("{} lines", total)
            };
            println!("  - {} ({})", file, lines_display);
        }
        println!();

        print!(
            "{}",
            "Would you like to add them to .cmtignore? [Y/n] ".cyan()
        );
        let _ = io::stdout().flush();

        let mut input = String::new();
        let should_add = if io::stdin().read_line(&mut input).is_ok() {
            let input = input.trim().to_lowercase();
            input.is_empty() || input == "y" || input == "yes"
        } else {
            false
        };

        if should_add {
            let files_to_add: Vec<String> = staged
                .stats
                .skipped_files
                .iter()
                .map(|(f, _, _)| f.clone())
                .collect();

            match append_to_cmtignore(repo_root, &files_to_add) {
                Ok(()) => {
                    println!(
                        "{}",
                        "Added to .cmtignore. These files will be skipped for analysis in future runs."
                            .green()
                    );
                    println!(
                        "{}",
                        "(They will still be committed normally, just not sent to the LLM.)"
                            .dimmed()
                    );
                    println!();
                }
                Err(e) => {
                    eprintln!(
                        "{}",
                        format!("Warning: Failed to update .cmtignore: {}", e)
                            .yellow()
                            .bold()
                    );
                }
            }
        }
    }

    // Scrub likely secrets from the diff before it ever leaves the machine.
    let staged_changes = if config.redact {
        let (scrubbed, redacted) = cmt::redact_secrets(&staged.diff_text);
        if redacted > 0 && !config.message_only {
            eprintln!(
                "{}",
                format!(
                    "🔒 Redacted {} likely secret{} from the diff before sending it to the model (disable with --no-redact).",
                    redacted,
                    if redacted == 1 { "" } else { "s" }
                )
                .yellow()
            );
        }
        scrubbed
    } else {
        staged.diff_text.clone()
    };

    // Determine diff size for adaptive behaviors (very high thresholds - Gemini supports 1M tokens)
    let is_very_large_diff = staged.stats.files_changed > 150
        || (staged.stats.insertions + staged.stats.deletions) > 50000;

    // Get recent commits - only skip for extremely large diffs
    let include_recent = config.include_recent_commits && !is_very_large_diff;
    let effective_recent_count = if include_recent {
        config.recent_commits_count // Always use full count - we have the token budget
    } else {
        0
    };

    if config.include_recent_commits && !include_recent {
        eprintln!(
            "{}",
            "Skipping recent commits for this extremely large diff.".yellow()
        );
    }

    let recent_commits = if include_recent {
        match cmt::get_recent_commits(&repo, effective_recent_count) {
            Ok(commits) => commits,
            Err(e) => {
                eprintln!(
                    "{}",
                    "Warning: Failed to get recent commits:".yellow().bold()
                );
                eprintln!("{}", e);
                String::new()
            }
        }
    } else {
        String::new()
    };

    // Get current branch name for context
    let branch_name = get_current_branch(&repo);

    // Get README excerpt for project context (first 50 lines)
    let readme_excerpt = get_readme_excerpt(&repo, 50);

    // Show raw diff if requested
    if config.show_raw_diff {
        println!("{}", "Raw diff:".cyan().bold());
        println!("{}", staged_changes);
        println!();
    }

    // Get model info for display
    let model_name = config
        .model
        .clone()
        .unwrap_or_else(|| default_model(&config.provider).to_string());

    // Show diff stats before sending to LLM (unless message-only mode)
    if !config.message_only && !config.no_diff_stats {
        staged.stats.print();
    }

    // Generate commit message with spinner (only when attached to a terminal;
    // don't animate into a pipe/log).
    let spinner = if !config.message_only && io::stdout().is_terminal() {
        Some(Spinner::new(&format!(
            "Generating commit message with {}...",
            model_name
        )))
    } else {
        None
    };

    let start_time = Instant::now();
    let result = match generate_commit_message(
        &config,
        &staged_changes,
        &recent_commits,
        branch_name.as_deref(),
        readme_excerpt.as_deref(),
        &template_manager,
    )
    .await
    {
        Ok(result) => {
            if let Some(s) = &spinner {
                s.finish_and_clear();
            }
            result
        }
        Err(e) => {
            if let Some(s) = &spinner {
                s.finish_and_clear();
            }
            eprintln!("{}", "Error generating commit message:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };
    let elapsed = start_time.elapsed();
    let commit_message = result.message;

    // Copy to clipboard if requested
    if args.copy {
        match Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(&commit_message) {
                    eprintln!(
                        "{}",
                        format!("Warning: Failed to copy to clipboard: {}", e)
                            .yellow()
                            .bold()
                    );
                } else if !config.message_only {
                    println!("{}", "✓ Copied to clipboard".green());
                }
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("Warning: Failed to access clipboard: {}", e)
                        .yellow()
                        .bold()
                );
            }
        }
    }

    // Output the commit message
    if config.message_only {
        // Output just the message for piping to git commit
        print!("{}", commit_message);
    } else {
        // Show the generated commit message
        println!("{}", "Commit message:".green().bold());
        println!("{}", commit_message);

        // Use actual token counts from API, or estimate if not available
        let (input_tokens, output_tokens) = match (result.input_tokens, result.output_tokens) {
            (Some(input), Some(output)) => (input, output),
            _ => {
                // Fallback: estimate ~4 chars per token
                let est_input = (staged_changes.len() + recent_commits.len()) as u64 / 4;
                let est_output = commit_message.len() as u64 / 4;
                (est_input, est_output)
            }
        };
        let total_tokens = input_tokens + output_tokens;
        let elapsed_secs = elapsed.as_secs_f32();

        let cost_str = pricing_cache
            .get_model_pricing(&config.provider, &model_name)
            .and_then(|p| pricing::calculate_cost(&p, input_tokens, output_tokens))
            .map(|c| format!(", {}", pricing::format_cost(c)))
            .unwrap_or_default();

        // Show ~ prefix only if we're estimating
        let token_prefix = if result.input_tokens.is_some() {
            ""
        } else {
            "~"
        };
        println!(
            "{}",
            format!(
                "{}{} tokens, {:.1}s{}",
                token_prefix, total_tokens, elapsed_secs, cost_str
            )
            .dimmed()
        );

        // Handle commit prompt (default behavior unless --no-commit)
        if !args.no_commit && !args.yes && !interactive {
            // No TTY to confirm on, and -y was not passed: don't silently cancel.
            eprintln!(
                "{}",
                "Not committing: stdin is not a terminal. Re-run with -y to commit, or --no-commit to just print the message."
                    .yellow()
            );
        } else if !args.no_commit {
            let mut current_message = commit_message.clone();
            // Clone the resolved config so hint regeneration can layer in a hint
            // without mutating the original.
            let mut current_config = config.clone();

            loop {
                let action = if args.yes {
                    CommitAction::Commit
                } else {
                    // Prompt for action
                    print!(
                        "{}",
                        "[y]es to commit, [e]dit, [n]o to cancel, [h]int to regenerate: ".cyan()
                    );
                    let _ = io::stdout().flush();

                    let mut input = String::new();
                    if io::stdin().read_line(&mut input).is_ok() {
                        let input = input.trim().to_lowercase();
                        match input.as_str() {
                            "y" | "yes" => CommitAction::Commit,
                            "e" | "edit" => CommitAction::Edit,
                            "n" | "no" | "" => CommitAction::Cancel,
                            "h" | "hint" => CommitAction::Hint,
                            _ => CommitAction::Cancel,
                        }
                    } else {
                        CommitAction::Cancel
                    }
                };

                match action {
                    CommitAction::Commit => {
                        // Create the commit using git commit (respects hooks)
                        let options = CommitOptions {
                            no_verify: args.no_verify,
                        };
                        match create_commit(&repo, &current_message, &options) {
                            Ok(result) => {
                                println!(
                                    "{}",
                                    format!("✓ Created commit: {}", &result.oid[..7])
                                        .green()
                                        .bold()
                                );
                            }
                            Err(err @ CommitError::PreCommitFailed { .. }) => {
                                eprintln!("{}", "Pre-commit hook failed.".red().bold());
                                if let Some(output) = err.hook_output() {
                                    eprintln!();
                                    eprintln!("{}", output);
                                }
                                eprintln!("{}", "Use --no-verify (-n) to skip hooks.".yellow());
                                process::exit(1);
                            }
                            Err(err @ CommitError::CommitMsgFailed { .. }) => {
                                eprintln!("{}", "Commit-msg hook failed.".red().bold());
                                if let Some(output) = err.hook_output() {
                                    eprintln!();
                                    eprintln!("{}", output);
                                }
                                eprintln!("{}", "Use --no-verify (-n) to skip hooks.".yellow());
                                process::exit(1);
                            }
                            Err(e) => {
                                eprintln!("{}", "Error creating commit:".red().bold());
                                eprintln!("{}", e);
                                process::exit(1);
                            }
                        }
                        break;
                    }
                    CommitAction::Cancel => {
                        println!("{}", "Commit cancelled.".yellow());
                        break;
                    }
                    CommitAction::Edit => match edit_in_editor(&current_message) {
                        Some(edited) => {
                            current_message = edited;
                            println!();
                            println!("{}", "Commit message:".green().bold());
                            println!("{}", current_message);
                        }
                        None => {
                            eprintln!(
                                "{}",
                                "Edit discarded (empty or editor failed); keeping the message."
                                    .yellow()
                            );
                        }
                    },
                    CommitAction::Hint => {
                        // Prompt for hint
                        print!("{}", "Enter hint: ".cyan());
                        let _ = io::stdout().flush();

                        let mut hint_input = String::new();
                        if io::stdin().read_line(&mut hint_input).is_ok() {
                            let hint = hint_input.trim();
                            if !hint.is_empty() {
                                current_config.hint = Some(hint.to_string());

                                // Regenerate with spinner
                                let spinner =
                                    Spinner::new(&format!("Regenerating with {}...", model_name));
                                match generate_commit_message(
                                    &current_config,
                                    &staged_changes,
                                    &recent_commits,
                                    branch_name.as_deref(),
                                    readme_excerpt.as_deref(),
                                    &template_manager,
                                )
                                .await
                                {
                                    Ok(new_result) => {
                                        spinner.finish_and_clear();
                                        current_message = new_result.message;
                                        println!();
                                        println!("{}", "Commit message:".green().bold());
                                        println!("{}", current_message);
                                    }
                                    Err(e) => {
                                        spinner.finish_and_clear();
                                        eprintln!(
                                            "{}",
                                            "Error regenerating commit message:".red().bold()
                                        );
                                        eprintln!("{}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
