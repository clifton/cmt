use cmt::ai_mod::create_default_registry;
use cmt::config_mod::{file as config_file, Config};
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
