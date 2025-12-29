use arboard::Clipboard;
use cmt::ai_mod::create_default_registry;
use cmt::config_mod::{file as config_file, Config};
use cmt::template_mod::TemplateManager;
use cmt::{analyze_diff, generate_commit_message, git_staged_changes, Args, Spinner};
use colored::*;
use dotenv::dotenv;
use git2::Repository;
use std::io::{self, Write};
use std::{env, process};

fn main() {
    dotenv().ok(); // Load .env file if it exists
    let args = Args::new_from(env::args());

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

    // Get staged changes
    let staged_changes = match cmt::get_staged_changes(
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

    // Get recent commits if enabled
    let recent_commits = if args.include_recent_commits {
        match cmt::get_recent_commits(&repo, args.recent_commits_count) {
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

    // Generate commit message with spinner (only in interactive mode)
    let spinner = if !args.message_only {
        Some(Spinner::new("Generating commit message..."))
    } else {
        None
    };

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
        // Show diff stats if not disabled
        if !args.no_diff_stats {
            match git_staged_changes(&repo) {
                Ok(_) => {
                    // The function already prints the stats, no need to print again
                    println!(); // Add an extra newline for spacing
                }
                Err(e) => {
                    eprintln!(
                        "{}",
                        "Warning: Failed to show diff statistics:".yellow().bold()
                    );
                    eprintln!("{}", e);
                }
            }
        }

        // Create registry to get default model
        let registry = create_default_registry();
        let default_model = registry.default_model_for(&args.provider);

        // Show which provider and model is being used
        println!(
            "{}",
            format!(
                "Using {} {}",
                args.provider,
                args.model.as_deref().unwrap_or(&default_model)
            )
            .cyan()
            .italic()
        );
        println!();

        // Show the generated commit message
        println!("{}", "Commit message:".green().bold());
        println!("{}", commit_message);

        // Handle direct commit if requested
        if args.commit {
            let should_commit = if args.yes {
                true
            } else {
                // Prompt for confirmation
                println!();
                print!(
                    "{}",
                    "Do you want to commit with this message? [y/N] ".cyan()
                );
                io::stdout().flush().unwrap();

                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_ok() {
                    let input = input.trim().to_lowercase();
                    input == "y" || input == "yes"
                } else {
                    false
                }
            };

            if should_commit {
                // Create the commit using git2
                match create_commit(&repo, &commit_message) {
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
            } else {
                println!("{}", "Commit cancelled.".yellow());
            }
        } else {
            // Show usage hint
            println!();
            println!("{}", "To use this message with git commit:".cyan());
            println!("git commit -F <(cmt --message-only)");
            println!(
                "{}",
                "Or use --commit to commit directly with confirmation.".cyan()
            );
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
