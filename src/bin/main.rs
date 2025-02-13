use cmt::{generate_commit_message, git_staged_changes, Args};
use colored::*;
use dotenv::dotenv;
use git2::Repository;
use std::{env, process};

/// Check if we're running via `cargo run`
fn is_cargo_run() -> bool {
    std::env::args()
        .next()
        .map(|arg| arg.contains("target/debug/") || arg.contains("target/release/"))
        .unwrap_or(false)
}

fn main() {
    dotenv().ok(); // Load .env file if it exists
    let args = Args::new_from(env::args());

    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("{}", "Error opening git repository:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    let staged_changes = match cmt::get_staged_changes(&repo, args.context_lines) {
        Ok(changes) => changes,
        Err(e) => {
            eprintln!("{}", "Error:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    if args.show_raw_diff {
        println!("\n{}", "Raw Git Diff:".blue().bold());
        println!();
        // Format each line to match git diff style
        for line in staged_changes.lines() {
            if line.starts_with("diff --git") {
                println!("{}", line.bold());
            } else if line.starts_with("index ") {
                println!("{}", line.yellow());
            } else if line.starts_with("+++") || line.starts_with("+") {
                println!("{}", line.green());
            } else if line.starts_with("---") || line.starts_with("-") {
                println!("{}", line.red());
            } else if line.starts_with("@@") {
                println!("{}", line.cyan());
            } else if line.starts_with(" ") {
                println!("{}", line); // Context lines already have a space
            } else {
                println!(" {}", line); // Add space for lines without prefix
            }
        }
        println!();
    }

    match generate_commit_message(&staged_changes, &args) {
        Ok(commit_message) => {
            if args.message_only {
                // When used with git commit -F, only output the message
                print!("{}", commit_message);
            } else {
                // Interactive mode - show full formatted output
                if !args.no_diff_stats {
                    if let Err(e) = git_staged_changes(&repo) {
                        eprintln!("Failed to show diff statistics: {}", e);
                    }
                }

                println!("\n{}", "Generated commit message:".green().bold());
                println!("{}", "-".repeat(30));
                println!("{}", commit_message);
                println!("{}", "-".repeat(30));

                println!("\nTo use this message, run:");
                let hint_arg = args.hint.as_ref().map_or(String::new(), |h| {
                    format!(" --hint '{}'", h.replace("'", "'\\''"))
                });

                if is_cargo_run() {
                    println!(
                        "git commit -F <(cargo run --quiet -- --message-only{})",
                        hint_arg
                    );
                } else {
                    println!("git commit -F <(cmt --message-only{})", hint_arg);
                }
            }
        }
        Err(e) => {
            eprintln!("{}", "Error generating commit message:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}
