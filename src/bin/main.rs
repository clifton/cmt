use cmt::ai_mod::create_default_registry;
use cmt::config_mod::{file as config_file, Config};
use cmt::template_mod::TemplateManager;
use cmt::{generate_commit_message, git_staged_changes, Args};
use colored::*;
use dotenv::dotenv;
use git2::Repository;
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

    // Show raw diff if requested
    if args.show_raw_diff {
        println!("{}", "Raw diff:".cyan().bold());
        println!("{}", staged_changes);
        println!();
    }

    // Generate commit message
    let commit_message = match generate_commit_message(&args, &staged_changes, &recent_commits) {
        Ok(message) => message,
        Err(e) => {
            eprintln!("{}", "Error generating commit message:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

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

        // Show usage hint
        println!();
        println!("{}", "To use this message with git commit:".cyan());
        println!("git commit -F <(cmt --message-only)");
    }
}
