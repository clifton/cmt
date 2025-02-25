# cmt - AI-Powered Git Commit Message Generator

`cmt` is a command-line tool that generates meaningful git commit messages using AI models. It analyzes your staged changes and generates a well-formatted, descriptive commit message following conventional commit standards.

## Features

- 🤖 Supports multiple AI models:
  - Anthropic's Claude Sonnet 3.7 (default)
  - OpenAI's GPT-4o
- 📝 Follows conventional commit format (`type: subject`)
- 💡 Contextual hints to guide message generation

## Installation

### Using Install Script (Recommended)

The easiest way to install `cmt` is using our install script:

```bash
curl -sSL https://raw.githubusercontent.com/clifton/cmt/main/scripts/install.sh | sh
```

This will automatically download and install the latest version for your platform.

### Installing from crates.io

You can also install `cmt` directly from crates.io:

```bash
cargo install cmt
```

### Installing from source

Alternatively, you can build from source:

```bash
# Clone the repository
git clone https://github.com/clifton/cmt.git
cd cmt

# Build and install
cargo install --path .
```

### Configuration

Set your API key(s) either:
1. As environment variables:
   ```bash
   # For Claude (default)
   export ANTHROPIC_API_KEY='your-api-key'

   # For OpenAI (optional)
   export OPENAI_API_KEY='your-api-key'
   ```
2. Or in a `.env` file in your project directory:
   ```
   ANTHROPIC_API_KEY=your-api-key
   OPENAI_API_KEY=your-api-key
   ```

## Usage

### Basic Usage

```bash
# Stage your changes first
git add .

# Generate a commit message using Claude (default)
cmt

# Generate a commit message using OpenAI
cmt --provider openai

# Use the generated message directly with git
git commit -F <(cmt --message-only)
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
          Number of context lines to show in the git diff [default: 12]
      --model <MODEL>
          Use a specific AI model (defaults to claude-3-7-sonnet-latest or gpt-4o depending on provider)
  -t, --temperature <TEMPERATURE>
          Adjust the creativity of the generated message (0.0 to 2.0)
      --hint <HINT>
          Add a hint to guide the AI in generating the commit message
      --max-lines-per-file <MAX_LINES_PER_FILE>
          Number of maximum lines to show per file in the git diff [default: 500]
      --max-line-width <MAX_LINE_WIDTH>
          Maximum line width for diffs [default: 300]
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
      --include-recent-commits
          Include recent commits for context
      --recent-commits-count <RECENT_COMMITS_COUNT>
          Number of recent commits to include for context [default: 5]
      --init-config
          Create a new configuration file
      --config-path <CONFIG_PATH>
          Path to save the configuration file (defaults to .cmt.toml in current directory)
      --provider <PROVIDER>
          Use a specific provider (claude, openai, etc.) [default: claude]
  -h, --help
          Print help
  -V, --version
          Print version
```

### Examples

```bash
# Generate a commit message with diff statistics (default)
cmt

# Show the raw git diff that will be sent to the AI
cmt --show-raw-diff

# Generate a commit message without diff statistics
cmt --no-diff-stats

# Use OpenAI with a custom temperature
cmt --provider openai --temperature 0.8

# Provide a hint for context
cmt --hint "This fixes the login timeout issue"

# List all available templates
cmt --list-templates

# Show the content of a specific template
cmt --show-template conventional

# Create a custom template
cmt --create-template my-template --template-content "{{type}}: {{subject}}\n\n{{details}}"

# Use a specific template
cmt --template detailed

# Combine multiple options
cmt --provider openai --model gpt-4o --hint "Update dependencies for security"

# Use with git commit directly
git commit -F <(cmt --message-only --hint "Refactor to improve performance")
```

## How It Works

1. When you run `cmt`, it analyzes your staged git changes
2. The changes are sent to the selected AI model (Claude or OpenAI) along with:
   - A system prompt that guides the model to generate conventional commits
   - Your optional hint for additional context
   - The staged changes as the user prompt
3. The AI generates a commit message following the conventional commit format
4. The message is displayed (with optional diff statistics) or output directly for use with git

## Template Management

`cmt` supports customizable templates for formatting commit messages. Templates use the Handlebars templating language.

### Available Templates

By default, `cmt` comes with three built-in templates:
- `conventional` (default): Standard conventional commit format
- `simple`: A simplified format without the commit type
- `detailed`: Extended format with support for breaking changes and issue references

You can list all available templates with:
```bash
cmt --list-templates
```

### Creating Custom Templates

You can create your own templates in the `~/.config/cmt/templates/` directory:

```bash
# Create a custom template
cmt --create-template my-template --template-content "{{type}}: {{subject}}\n\n{{details}}"
```

Templates are stored as `.hbs` files and can use the following variables:
- `{{type}}`: The commit type (feat, fix, docs, etc.)
- `{{subject}}`: The commit subject line
- `{{details}}`: The detailed description of changes
- `{{scope}}`: The scope of the change (optional)
- `{{breaking}}`: Breaking change information (optional)
- `{{issues}}`: Related issue references (optional)

### Using Templates

To use a specific template:
```bash
cmt --template my-template
```

You can also set a default template in your `.cmt.toml` configuration file:
```toml
template = "my-template"
```

## Commit Message Format

The generated commit messages follow the conventional commit format:

```
type: subject

- Detailed change 1
- Detailed change 2
```

Where `type` is one of:
- `feat`: New features
- `fix`: Bug fixes
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or modifying tests
- `chore`: Maintenance tasks

## Development

### Building from source

```bash
# Clone the repository
git clone https://github.com/yourusername/cmt.git
cd cmt

# Build
cargo build

# Run tests
cargo test

# Run in development
cargo run
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

