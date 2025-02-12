# cmt - AI-Powered Git Commit Message Generator

`cmt` is a command-line tool that generates meaningful git commit messages using OpenAI's GPT models. It analyzes your staged changes and generates a well-formatted, descriptive commit message following conventional commit standards.

## Features

- 🤖 Uses OpenAI's GPT models to generate contextual commit messages
- 📝 Follows conventional commit format (`type: subject`)
- 🎨 Beautiful colored output in interactive mode
- 📊 Optional diff statistics
- ⚙️ Configurable AI model and parameters
- 🔑 Supports environment variables for API key management

## Installation

### Prerequisites

- Rust and Cargo (install from [rustup.rs](https://rustup.rs))
- An OpenAI API key

### Installing from crates.io

The easiest way to install `cmt` is directly from crates.io:

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

Set your OpenAI API key either:
1. As an environment variable:
   ```bash
   export OPENAI_API_KEY='your-api-key'
   ```
2. Or in a `.env` file in your project directory:
   ```
   OPENAI_API_KEY=your-api-key
   ```

## Usage

### Basic Usage

```bash
# Stage your changes first
git add .

# Generate a commit message
cmt

# Use the generated message directly with git
git commit -F <(cmt --message-only)
```

### Command-line Options

```
Usage: cmt [OPTIONS]

Options:
  -m, --message-only        Only output the generated message, without formatting
  -s, --show-diff          Show the diff statistics
      --model <MODEL>      Use a different OpenAI model [default: gpt-4o]
  -t, --temperature <TEMP> Adjust the creativity of the message (0.0 to 2.0) [default: 1.0]
  -h, --help              Show this help message
  -V, --version           Show version information
```

### Examples

```bash
# Show diff statistics along with the message
cmt --show-diff

# Use a different model with lower creativity
cmt --model gpt-3.5-turbo --temperature 0.5

# Generate just the message (useful for scripts)
cmt --message-only
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

### Dependencies

- `git2`: Git operations
- `reqwest`: HTTP client for OpenAI API
- `clap`: Command-line argument parsing
- `colored`: Terminal colors
- `serde`: JSON serialization
- `dotenv`: Environment variable management

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.