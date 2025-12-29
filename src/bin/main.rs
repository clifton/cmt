use arboard::Clipboard;
use cmt::ai_mod::create_default_registry;
use cmt::config_mod::{file as config_file, Config};
use cmt::pricing::{self, PricingCache};
use cmt::template_mod::TemplateManager;
use cmt::{analyze_diff, generate_commit_message, Args, Spinner};
use colored::*;
use dotenv::dotenv;
use git2::Repository;
use std::io::{self, Write};
use std::time::Instant;
use std::{env, process};

enum CommitAction {
    Commit,
    Cancel,
    Hint,
}

fn main() {
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

    // Handle listing available templates
    if args.list_templates {
        match TemplateManager::new() {
            Ok(manager) => {
                println!("{}", "Available templates:".green().bold());
                for template in manager.list_templates() {
                    println!("- {}", template);
                }
                process::exit(0);
            }
            Err(e) => {
                eprintln!("{}", "Error listing templates:".red().bold());
                eprintln!("{}", e);
                process::exit(1);
            }
        }
    }

    // Handle listing available models
    if args.list_models {
        let registry = create_default_registry();
        let provider_name = &args.provider;

        match registry.get(provider_name) {
            Some(provider) => {
                match provider.fetch_available_models() {
                    Ok(models) => {
                        println!(
                            "{}",
                            format!("Available models for {}:", provider_name)
                                .green()
                                .bold(),
                        );

                        // Sort models alphabetically for better readability
                        let mut sorted_models = models.clone();
                        sorted_models.sort();

                        for model in sorted_models {
                            // Highlight the default model
                            if model == provider.default_model() {
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
            None => {
                eprintln!(
                    "{}",
                    format!("Provider '{}' not found", provider_name)
                        .red()
                        .bold()
                );
                eprintln!(
                    "Available providers: {}",
                    registry.provider_names().join(", ")
                );
                process::exit(1);
            }
        }
    }

    // Handle showing template content
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

    // Handle creating a new template
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

    // Open git repository
    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("{}", "Error opening git repository:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // Get staged changes (includes both diff text and stats in one pass)
    let staged = match cmt::get_staged_changes(
        &repo,
        args.context_lines,
        args.max_lines_per_file,
        args.max_line_width,
    ) {
        Ok(changes) => changes,
        Err(e) => {
            eprintln!("{}", "Error:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };
    let staged_changes = staged.diff_text.clone();

    // Determine diff size for adaptive behaviors
    let is_large_diff = staged.stats.files_changed > 40
        || (staged.stats.insertions + staged.stats.deletions) > 4000;

    // Get recent commits if enabled (adaptive gating and count capping on large diffs)
    let include_recent = !args.no_recent_commits && !is_large_diff;
    let effective_recent_count = if include_recent {
        if staged.stats.files_changed > 25
            || (staged.stats.insertions + staged.stats.deletions) > 2000
        {
            args.recent_commits_count.min(3)
        } else {
            args.recent_commits_count
        }
    } else {
        0
    };

    if !args.no_recent_commits && !include_recent {
        eprintln!(
            "{}",
            "Skipping recent commits for this large diff to reduce prompt size.".yellow()
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

    // Analyze the diff for better commit type classification
    let analysis = match analyze_diff(&repo) {
        Ok(a) => Some(a),
        Err(e) => {
            eprintln!("{}", "Warning: Failed to analyze diff:".yellow().bold());
            eprintln!("{}", e);
            None
        }
    };

    // Show raw diff if requested
    if args.show_raw_diff {
        println!("{}", "Raw diff:".cyan().bold());
        println!("{}", staged_changes);
        if let Some(ref a) = analysis {
            println!("\n{}", "Diff analysis:".cyan().bold());
            println!("{}", a.summary());
        }
        println!();
    }

    // Get provider and model info for display
    let registry = create_default_registry();
    let model_name = args
        .model
        .clone()
        .unwrap_or_else(|| registry.default_model_for(&args.provider));

    // Show diff stats before sending to LLM (unless message-only mode)
    if !args.message_only && !args.no_diff_stats {
        staged.stats.print();
    }

    // Generate commit message with spinner (only in interactive mode)
    let spinner = if !args.message_only {
        Some(Spinner::new(&format!(
            "Generating commit message with {}...",
            model_name
        )))
    } else {
        None
    };

    let start_time = Instant::now();
    let commit_message =
        match generate_commit_message(&args, &staged_changes, &recent_commits, analysis.as_ref()) {
            Ok(message) => {
                if let Some(s) = &spinner {
                    s.finish_and_clear();
                }
                message
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
                } else if !args.message_only {
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
    if args.message_only {
        // Output just the message for piping to git commit
        print!("{}", commit_message);
    } else {
        // Show the generated commit message
        println!("{}", "Commit message:".green().bold());
        println!("{}", commit_message);

        // Show stats: tokens, time, cost
        // Rough estimate: ~4 chars per token
        let input_tokens = (staged_changes.len() + recent_commits.len()) / 4;
        let output_tokens = commit_message.len() / 4;
        let total_tokens = input_tokens + output_tokens;
        let elapsed_secs = elapsed.as_secs_f32();

        let cost_str = pricing_cache
            .get_model_pricing(&args.provider, &model_name)
            .and_then(|p| pricing::calculate_cost(&p, input_tokens as u64, output_tokens as u64))
            .map(|c| format!(", {}", pricing::format_cost(c)))
            .unwrap_or_default();

        println!(
            "{}",
            format!("~{} tokens, {:.1}s{}", total_tokens, elapsed_secs, cost_str).dimmed()
        );

        // Handle commit prompt (default behavior unless --no-commit)
        if !args.no_commit {
            let mut current_message = commit_message.clone();
            let mut current_args = args.clone();

            loop {
                let action = if current_args.yes {
                    CommitAction::Commit
                } else {
                    // Prompt for action
                    print!(
                        "{}",
                        "[y]es to commit, [n]o to cancel, [h]int to regenerate: ".cyan()
                    );
                    io::stdout().flush().unwrap();

                    let mut input = String::new();
                    if io::stdin().read_line(&mut input).is_ok() {
                        let input = input.trim().to_lowercase();
                        match input.as_str() {
                            "y" | "yes" => CommitAction::Commit,
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
                        // Create the commit using git2
                        match create_commit(&repo, &current_message) {
                            Ok(oid) => {
                                println!(
                                    "{}",
                                    format!("✓ Created commit: {}", &oid.to_string()[..7])
                                        .green()
                                        .bold()
                                );
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
                    CommitAction::Hint => {
                        // Prompt for hint
                        print!("{}", "Enter hint: ".cyan());
                        io::stdout().flush().unwrap();

                        let mut hint_input = String::new();
                        if io::stdin().read_line(&mut hint_input).is_ok() {
                            let hint = hint_input.trim();
                            if !hint.is_empty() {
                                current_args.hint = Some(hint.to_string());

                                // Regenerate with spinner
                                let spinner =
                                    Spinner::new(&format!("Regenerating with {}...", model_name));
                                match generate_commit_message(
                                    &current_args,
                                    &staged_changes,
                                    &recent_commits,
                                    analysis.as_ref(),
                                ) {
                                    Ok(new_message) => {
                                        spinner.finish_and_clear();
                                        current_message = new_message;
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

/// Create a commit with the given message
fn create_commit(repo: &Repository, message: &str) -> Result<git2::Oid, git2::Error> {
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let signature = repo.signature()?;

    // Get parent commit (if any)
    let parents = match repo.head() {
        Ok(head) => {
            let parent = head.peel_to_commit()?;
            vec![parent]
        }
        Err(_) => vec![], // Initial commit
    };

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parent_refs,
    )
}
