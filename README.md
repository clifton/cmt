# cmt - AI-Powered Git Commit Message Generator

`cmt` is a command-line tool that generates meaningful git commit messages using AI models. It analyzes your staged changes and generates a well-formatted, descriptive commit message following conventional commit standards.

<img width="489" height="317" alt="image" src="https://github.com/user-attachments/assets/91625584-ad80-48cd-a607-be79c7cf7832" />

## Features

- 🤖 Supports multiple AI providers:
  - Google's Gemini (`gemini-3-flash-preview`, default - fastest & cheapest)
  - Anthropic's Claude (`claude-sonnet-4-5-20250929`)
  - OpenAI's GPT (`gpt-5.2`)
- 📝 Follows conventional commit format (`type: subject`)
- 🧠 Rich context: README, branch name, recent commits, full diff analysis
- 💡 Contextual hints to guide message generation
- ✅ Interactive commit prompt by default
- 📋 Copy to clipboard with `-c/--copy`
- ⚡ Configurable reasoning depth (none/minimal/low/high)
- 💰 Shows estimated token usage, time, and cost

## Installation

### Homebrew (macOS/Linux)

```bash
brew install clifton/tap/cmt
```

### Using Install Script

```bash
curl -sSL https://raw.githubusercontent.com/clifton/cmt/main/scripts/install.sh | sh
```

### From crates.io

```bash
cargo install cmt
```

### From source

```bash
git clone https://github.com/clifton/cmt.git
cd cmt
cargo install --path .
```

## Configuration

Set your API key as an environment variable:

```bash
# For Gemini (default provider)
export GEMINI_API_KEY='your-api-key'

# For Claude (optional)
export ANTHROPIC_API_KEY='your-api-key'

# For OpenAI (optional)
export OPENAI_API_KEY='your-api-key'
```

Or create a `.env` file in your project directory.

## Usage

### Basic Usage

```bash
# Stage your changes
git add .

# Generate commit message and prompt to commit (default)
cmt

# Just show the message without committing
cmt --no-commit

# Commit without confirmation prompt
cmt -y
```

### Providers

```bash
# Use Gemini (default)
cmt

# Use Claude
cmt --provider claude

# Use OpenAI
cmt --provider openai

# List available models for a provider
cmt --provider openai --list-models
```

### Command-line Options

```
CLI tool that generates commit messages using AI

Usage: cmt [OPTIONS]

Options:
  -m, --message-only
          Only output the generated commit message, without formatting
      --no-diff-stats
          Hide the diff statistics for staged changes
      --show-raw-diff
          Show the raw git diff that will be sent to the AI model
      --context-lines <CONTEXT_LINES>
          Number of context lines to show in the git diff [default: 20]
      --model <MODEL>
          Use a specific AI model (defaults to gemini-3-flash-preview,
          claude-sonnet-4-5-20250929, or gpt-5.2 depending on provider)
      --list-models
          List available models for the selected provider
  -t, --temperature <TEMPERATURE>
          Adjust the creativity of the generated message (0.0 to 2.0)
      --hint <HINT>
          Add a hint to guide the AI in generating the commit message
      --max-lines-per-file <MAX_LINES_PER_FILE>
          Number of maximum lines to show per file in the git diff [default: 2000]
      --max-line-width <MAX_LINE_WIDTH>
          Maximum line width for diffs [default: 500]
      --template <TEMPLATE>
          Use a specific template for the commit message
      --list-templates
          List all available templates
      --create-template <CREATE_TEMPLATE>
          Create a new template
      --template-content <TEMPLATE_CONTENT>
          Content for the new template (used with --create-template)
      --show-template <SHOW_TEMPLATE>
          Show the content of a specific template
      --no-recent-commits
          Disable including recent commits for context
      --recent-commits-count <RECENT_COMMITS_COUNT>
          Number of recent commits to include for context [default: 10]
      --init-config
          Create a new configuration file
      --config-path <CONFIG_PATH>
          Path to save the configuration file (defaults to .cmt.toml in current directory)
      --provider <PROVIDER>
          Use a specific provider (gemini, claude, openai) [default: gemini]
  -c, --copy
          Copy the generated commit message to clipboard
      --no-commit
          Skip commit prompt (just show the message)
  -y, --yes
          Skip confirmation when committing
      --thinking <THINKING>
          Reasoning depth for AI models (none=fastest, minimal, low, high) [default: low]
  -h, --help
          Print help
  -V, --version
          Print version
```

### Examples

```bash
# Default: generate message and prompt to commit
cmt

# Provide context to improve the message
cmt --hint "This fixes the login timeout issue"

# Review message without committing
cmt --no-commit

# Copy message to clipboard
cmt --copy

# Fastest mode (no reasoning)
cmt --thinking none

# Default uses low reasoning for balanced speed/quality
cmt

# Commit immediately without prompting
cmt -y

# Use a different provider with custom temperature
cmt --provider openai -t 0.8

# Pipe message to git directly
git commit -F <(cmt -m)
```

## How It Works

1. `cmt` gathers rich context: README excerpt, branch name, recent commits
2. Analyzes staged changes to suggest commit type
3. Sends full context + diff to the AI using [rstructor](https://github.com/clifton/rstructor) for structured output
4. Shows stats (tokens, time, estimated cost)
5. You review and confirm (or regenerate with a hint)

> **Built with [rstructor](https://github.com/clifton/rstructor)** - a Rust library for type-safe structured outputs from LLMs (similar to Python's Instructor).

For detailed implementation documentation including diff assembly, file filtering, prompt construction, and all default parameters, see [METHODOLOGY.md](METHODOLOGY.md).

Example output:
```
Staged: 3 files +150 -42
  src/main.rs   +100 -20
  src/lib.rs    +30  -12
  Cargo.toml    +20  -10

Commit message:
feat: add user authentication endpoint

- Implement JWT token validation
- Add password hashing with bcrypt
- Create login and logout handlers

~1250 tokens, 1.2s, $0.0008

[y]es to commit, [n]o to cancel, [h]int to regenerate:
```

## Commit Message Format

```
type: subject

- Detail about change 1
- Detail about change 2
```

Scope (`type(scope): subject`) is only used for monorepos or large codebases with distinct modules.

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `perf`, `build`, `ci`

## Template Management

```bash
# List available templates
cmt --list-templates

# Use a specific template
cmt --template detailed

# Create a custom template
cmt --create-template my-template --template-content "{{type}}: {{subject}}"
```

Templates are stored in `~/.config/cmt/templates/` as `.hbs` files.

Available variables: `{{type}}`, `{{subject}}`, `{{details}}`, `{{scope}}`, `{{breaking}}`, `{{issues}}`

## License

MIT License - see LICENSE file for details.
