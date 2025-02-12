use clap::Parser;
use colored::*;
use dotenv::dotenv;
use git2::{Repository, StatusOptions};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, process};

/// A CLI tool that generates commit messages using OpenAI
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Only output the generated commit message, without formatting
    #[arg(short, long)]
    message_only: bool,

    /// Show the diff of staged changes
    #[arg(short, long)]
    show_diff: bool,

    /// Use a different OpenAI model (default: gpt-4o)
    #[arg(long, default_value = "gpt-4o")]
    model: String,

    /// Adjust the creativity of the generated message (0.0 to 2.0)
    #[arg(short, long, default_value_t = 1.0)]
    temperature: f32,
}

/// Check if we're running via `cargo run`
fn is_cargo_run() -> bool {
    env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .map(|name| name.contains("cmt-"))
        .unwrap_or(false)
}

fn get_staged_changes(repo: &Repository) -> String {
    let mut status_opts = StatusOptions::new();
    status_opts.include_untracked(false);

    let statuses = repo.statuses(Some(&mut status_opts)).unwrap_or_else(|e| {
        eprintln!("Failed to get repository status: {}", e);
        process::exit(1);
    });

    let mut changes = Vec::new();

    for entry in statuses.iter() {
        let status = entry.status();
        let is_staged = status.is_index_new()
            || status.is_index_modified()
            || status.is_index_deleted()
            || status.is_index_renamed()
            || status.is_index_typechange();

        if is_staged {
            let path = entry.path().unwrap_or("unknown path");
            let status_str = match status {
                s if s.is_index_new() => "added",
                s if s.is_index_modified() => "modified",
                s if s.is_index_deleted() => "deleted",
                s if s.is_index_renamed() => "renamed",
                s if s.is_index_typechange() => "type changed",
                _ => "changed",
            };
            changes.push(format!("{}: {}", status_str, path));
        }
    }

    changes.join("\n")
}

fn generate_commit_message(
    changes: &str,
    args: &Args,
) -> Result<String, Box<dyn std::error::Error>> {
    if changes.is_empty() {
        return Ok(String::from("No staged changes found"));
    }

    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let client = Client::new();

    let prompt = format!(
        "Generate a concise and descriptive git commit message for the following changes:\n\n{}\n\n\
        The commit message should be in the present tense, be specific but concise, and follow best practices. \
        Format the response as a commit message without quotes or prefixes.",
        changes
    );

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": args.model,
            "messages": [
                {
                    "role": "system",
                    "content": "Generate git commit messages based on git diff output according to the standard commit specification.

You must return only the commit message without any other text or quotes.
Ignore changes to lock files. Be very succinct.

Format of the Commit Message:
{type}: {subject}

Also include a list in markdown format of more detailed changes, max line length of 80 characters, with two newlines between the message.

Allowed Types:
- feat
- fix
- docs
- style
- refactor
- test
- chore

You are a helpful assistant that generates clear and concise git commit messages."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": args.temperature,
            "max_tokens": 100
        }))
        .send()?;

    let response_json: Value = response.json()?;
    let message = response_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("Failed to generate commit message")
        .trim()
        .to_string();

    Ok(message)
}

fn show_git_diff(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
    let diff = repo.diff_index_to_workdir(None, None)?;
    let stats = diff.stats()?;
    println!("\n{}", "Diff Statistics:".blue().bold());
    println!("Files changed: {}", stats.files_changed());
    println!("Insertions: {}", stats.insertions());
    println!("Deletions: {}", stats.deletions());
    Ok(())
}

fn main() {
    dotenv().ok(); // Load .env file if it exists
    let args = Args::parse();

    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("{}", "Error opening git repository:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    let staged_changes = get_staged_changes(&repo);

    if staged_changes.is_empty() {
        println!("{}", "No staged changes found.".yellow().bold());
        println!("Stage some changes first with 'git add <files>'");
        process::exit(1);
    }

    match generate_commit_message(&staged_changes, &args) {
        Ok(commit_message) => {
            if args.message_only {
                // When used with git commit -F, only output the message
                print!("{}", commit_message);
            } else {
                // Interactive mode - show full formatted output
                println!("{}", "\nStaged changes:".blue().bold());
                println!("{}", "-".repeat(30));
                println!("{}", staged_changes);
                println!("{}", "-".repeat(30));

                if args.show_diff {
                    if let Err(e) = show_git_diff(&repo) {
                        eprintln!("Failed to show diff: {}", e);
                    }
                }

                println!("\n{}", "Generated commit message:".green().bold());
                println!("{}", "-".repeat(30));
                println!("{}", commit_message);
                println!("{}", "-".repeat(30));

                println!("\nTo use this message, run:");
                if is_cargo_run() {
                    println!("git commit -F <(cargo run --quiet -- --message-only)");
                } else {
                    println!("git commit -F <(cmt --message-only)");
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
