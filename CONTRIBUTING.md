# Contributing to Spool

Thanks for your interest in contributing to Spool! This document outlines how to get started.

## Development Setup

### Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs))
- A terminal that supports 256 colors (for TUI development)

### Getting Started

```bash
# Clone the repo
git clone https://github.com/asbjornb/spool.git
cd spool

# Set up pre-commit hooks (runs fmt, clippy, tests on commit)
bash scripts/setup-hooks.sh

# Build all crates
cargo build

# Run tests
cargo test

# Run the CLI
cargo run --bin spool -- --help
```

## Project Structure

```
spool/
├── crates/
│   ├── spool-format/     # Core types and serialization
│   ├── spool-adapters/   # Agent log parsers
│   └── spool-cli/        # TUI application
├── docs/                 # Documentation
└── examples/             # Example .spool files
```

## Making Changes

### Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Write tests for new functionality

### Commit Messages

Use conventional commits:

```
feat: add Codex adapter
fix: handle empty thinking blocks
docs: update SPEC.md with subagent examples
test: add round-trip tests for redaction
```

### Pull Requests

1. Fork the repo
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Push and open a PR

## Areas to Contribute

### Good First Issues

- [ ] Add more redaction patterns (phone numbers, SSNs)
- [ ] Improve error messages
- [ ] Add more example `.spool` files
- [ ] Documentation improvements

### Adapters Needed

- [ ] Cursor
- [ ] Aider
- [ ] GitHub Copilot CLI
- [ ] Continue

### TUI Improvements

- [ ] Mouse support
- [ ] Better syntax highlighting
- [ ] Themes/color schemes

## Testing

### Unit Tests

```bash
cargo test -p spool-format
cargo test -p spool-adapters
```

### Integration Tests

```bash
cargo test -p spool-cli
```

### Manual Testing

For TUI development, it's helpful to have sample sessions:

```bash
# Use example files
cargo run --bin spool -- view examples/simple-session.spool
```

## Questions?

Open an issue or start a discussion. We're happy to help!

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
