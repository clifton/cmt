use colored::*;
use dotenv::dotenv;
use git2::{Repository, StatusOptions};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, process};

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

fn generate_commit_message(changes: &str) -> Result<String, Box<dyn std::error::Error>> {
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
            "model": "gpt-4o",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a helpful assistant that generates clear and concise git commit messages."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 1.0,
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

fn main() {
    dotenv().ok(); // Load .env file if it exists

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
        process::exit(0);
    }

    match generate_commit_message(&staged_changes) {
        Ok(commit_message) => {
            println!("{}", "\nStaged changes:".blue().bold());
            println!("{}", "-".repeat(30));
            println!("{}", staged_changes);
            println!("{}", "-".repeat(30));

            println!("\n{}", "Generated commit message:".green().bold());
            println!("{}", "-".repeat(30));
            println!("{}", commit_message);
            println!("{}", "-".repeat(30));

            println!("\nTo use this message, run:");
            println!("git commit -F <(cargo run)");
        }
        Err(e) => {
            eprintln!("{}", "Error generating commit message:".red().bold());
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}
