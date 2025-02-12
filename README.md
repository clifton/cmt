# cmt - AI-Powered Git Commit Message Generator

`cmt` is a command-line tool that generates meaningful git commit messages using AI models. It analyzes your staged changes and generates a well-formatted, descriptive commit message following conventional commit standards.

## Features

- 🤖 Supports multiple AI models:
  - Anthropic's Claude 3.5 Sonnet (default, temperature 0.3)
  - OpenAI's GPT-4 Optimized (temperature 1.0)
- 📝 Follows conventional commit format (`type: subject`)
- 🎨 Beautiful colored output in interactive mode
- 📊 Optional diff statistics
- 💡 Contextual hints to guide message generation
- ⚙️ Configurable AI model and parameters
- 🔑 Supports environment variables for API keys

## Installation

### Using Install Script (Recommended)

The easiest way to install `cmt` is using our install script:

```bash
curl -sSL https://raw.githubusercontent.com/cliftonk/cmt/main/scripts/install.sh | sh
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
git clone https://github.com/cliftonk/cmt.git
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
cmt --openai

# Use the generated message directly with git
git commit -F <(cmt --message-only)
```

### Command-line Options

```
Usage: cmt [OPTIONS]

Options:
  -m, --message-only               Only output the generated commit message, without formatting
  -s, --show-diff                  Show the diff of staged changes
      --model <MODEL>              Use a specific AI model (defaults to claude-3-5-sonnet-latest or gpt-4o depending on provider)
      --openai                     Use OpenAI instead of Claude (which is default)
      --anthropic                  Use Anthropic instead of OpenAI (which is default)
  -t, --temperature <TEMPERATURE>  Adjust the creativity of the generated message (0.0 to 2.0)
      --hint <HINT>                Add a hint to guide the AI in generating the commit message
  -h, --help                       Print help
  -V, --version                    Print version
```

### Examples

```bash
# Show diff statistics along with the message
cmt --show-diff

# Use OpenAI with a custom temperature
cmt --openai --temperature 0.8

# Provide a hint for context
cmt --hint "This fixes the login timeout issue"

# Combine multiple options
cmt --openai --model gpt-4 --hint "Update dependencies for security" --show-diff

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

### Dependencies

- `git2`: Git operations
- `reqwest`: HTTP client for OpenAI API
- `clap`: Command-line argument parsing
- `colored`: Terminal colors
- `serde`: JSON serialization
- `dotenv`: Environment variable management

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

Staged Changes:
1 file changed, 29 insertions(+), 18 deletions(-)
README.md | +++++++++++++++++++++++++++++------------------
