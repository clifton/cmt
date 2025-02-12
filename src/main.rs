use anthropic::types::MessagesRequest;
use anthropic::types::{ContentBlock, Message, Role};
use anthropic::{client::Client as Anthropic, config::AnthropicConfig};
use clap::Parser;
use colored::*;
use dotenv::dotenv;
use git2::{Repository, StatusOptions};
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

async fn generate_commit_message_claude(
    changes: &str,
    args: &Args,
) -> Result<String, Box<dyn std::error::Error>> {
    let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");
    let mut anthropic_config = AnthropicConfig::default();
    anthropic_config.api_key = api_key;
    let client = Anthropic::try_from(anthropic_config)?;

    let model = args
        .model
        .clone()
        .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string());

    let user_prompt = USER_PROMPT_TEMPLATE.replace("{}", changes);

    let message = Message {
        role: Role::User,
        content: vec![ContentBlock::Text { text: user_prompt }],
    };

    let response = client
        .messages(MessagesRequest {
            model,
            max_tokens: 100,
            temperature: Some(args.temperature.unwrap_or(CLAUDE_DEFAULT_TEMP) as f64),
            system: SYSTEM_PROMPT.to_string(),
            messages: vec![message],
            ..Default::default()
        })
        .await?;
    let content = response.content.first().unwrap();
    let text = match content {
        ContentBlock::Text { text } => text.clone(),
        _ => String::from("Received unexpected content block type from Claude API"),
    };

    Ok(text)
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
        tokio::runtime::Runtime::new()?.block_on(generate_commit_message_claude(changes, args))
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
