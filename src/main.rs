use clap::Parser;
use colored::*;
use dotenv::dotenv;
use git2::Repository;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, process};

mod prompts;
use prompts::{SYSTEM_PROMPT, USER_PROMPT_TEMPLATE};

const CLAUDE_DEFAULT_TEMP: f32 = 0.3;
const OPENAI_DEFAULT_TEMP: f32 = 1.0;

/// A CLI tool that generates commit messages using AI
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Only output the generated commit message, without formatting
    #[arg(short, long)]
    message_only: bool,

    /// Show the diff of staged changes
    #[arg(short, long)]
    show_diff: bool,

    /// Use a specific AI model (defaults to claude-3.5-sonnet-latest or gpt-4o depending on provider)
    #[arg(long)]
    model: Option<String>,

    /// Use OpenAI instead of Claude (which is default)
    #[arg(long, default_value_t = false)]
    openai: bool,

    /// Use Anthropic instead of OpenAI (which is default)
    #[arg(long, default_value_t = true)]
    anthropic: bool,

    /// Adjust the creativity of the generated message (0.0 to 2.0)
    #[arg(short, long)]
    temperature: Option<f32>,
}

/// Check if we're running via `cargo run`
fn is_cargo_run() -> bool {
    std::env::args()
        .next()
        .map(|arg| arg.contains("target/debug/") || arg.contains("target/release/"))
        .unwrap_or(false)
}

fn get_staged_changes(repo: &Repository) -> Result<String, git2::Error> {
    let mut opts = git2::DiffOptions::new();
    let tree = match repo.head().and_then(|head| head.peel_to_tree()) {
        Ok(tree) => tree,
        Err(_) => {
            // If there's no HEAD (new repo), use an empty tree
            repo.treebuilder(None)
                .and_then(|builder| builder.write())
                .and_then(|oid| repo.find_tree(oid))
                .map_err(|e| {
                    git2::Error::from_str(&format!("Failed to create empty tree: {}", e))
                })?
        }
    };

    let diff = repo
        .diff_tree_to_index(Some(&tree), None, Some(&mut opts))
        .map_err(|e| git2::Error::from_str(&format!("Failed to get repository diff: {}", e)))?;

    let mut diff_output = Vec::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        use git2::DiffLineType::*;
        match line.origin_value() {
            Addition | Deletion | Context => {
                diff_output.extend_from_slice(line.content());
            }
            _ => {}
        }
        true
    })
    .map_err(|e| git2::Error::from_str(&format!("Failed to format diff: {}", e)))?;

    let diff_str = String::from_utf8_lossy(&diff_output).to_string();

    if diff_str.is_empty() {
        Err(git2::Error::from_str(
            "No changes have been staged for commit",
        ))
    } else {
        Ok(diff_str)
    }
}

fn generate_commit_message_claude(
    changes: &str,
    args: &Args,
) -> Result<String, Box<dyn std::error::Error>> {
    let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");
    let client = Client::new();

    let model = args
        .model
        .clone()
        .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string());

    let user_prompt = USER_PROMPT_TEMPLATE.replace("{}", changes);

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&json!({
            "model": model,
            "max_tokens": 1024,
            "temperature": args.temperature.unwrap_or(CLAUDE_DEFAULT_TEMP),
            "system": SYSTEM_PROMPT,
            "messages": [{
                "role": "user",
                "content": user_prompt
            }]
        }))
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                format!("Request timed out: {}", e)
            } else if e.is_connect() {
                format!(
                    "Connection error: {}. Please check your internet connection.",
                    e
                )
            } else if let Some(status) = e.status() {
                format!("API error (status {}): {}", status, e)
            } else {
                format!("Request error: {}", e)
            }
        })?;

    if !response.status().is_success() {
        let error_text = response.text()?;
        return Err(format!("API returned error: {}", error_text).into());
    }

    let response_json: Value = response
        .json()
        .map_err(|e| format!("Failed to parse API response: {}", e))?;
    let message = response_json["content"][0]["text"]
        .as_str()
        .unwrap_or("Failed to generate commit message")
        .trim()
        .to_string();

    Ok(message)
}

fn generate_commit_message_openai(
    changes: &str,
    args: &Args,
) -> Result<String, Box<dyn std::error::Error>> {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let client = Client::new();

    let model = args.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
    let user_prompt = USER_PROMPT_TEMPLATE.replace("{}", changes);

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": SYSTEM_PROMPT
                },
                {
                    "role": "user",
                    "content": user_prompt
                }
            ],
            "temperature": args.temperature.unwrap_or(OPENAI_DEFAULT_TEMP),
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

fn generate_commit_message(
    changes: &str,
    args: &Args,
) -> Result<String, Box<dyn std::error::Error>> {
    if changes.is_empty() {
        return Ok(String::from("No staged changes found"));
    }

    if args.openai {
        generate_commit_message_openai(changes, args)
    } else {
        generate_commit_message_claude(changes, args)
    }
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

    let staged_changes = match get_staged_changes(&repo) {
        Ok(changes) => changes,
        Err(e) => {
            eprintln!("{}", "Error:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    match generate_commit_message(&staged_changes, &args) {
        Ok(commit_message) => {
            if args.message_only {
                // When used with git commit -F, only output the message
                print!("{}", commit_message);
            } else {
                // Interactive mode - show full formatted output
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
