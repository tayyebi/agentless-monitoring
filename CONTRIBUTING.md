# Contributing to Agentless Monitor

Thank you for your interest in contributing to Agentless Monitor! This document provides guidelines and information for contributors.

## 🚀 Getting Started

### Prerequisites

- Rust 1.70+ 
- Git
- SSH access to test servers (for testing)

### Development Setup

1. **Fork and clone the repository**
   ```bash
   git clone https://github.com/tayyebi/agentless-monitor.git
   cd agentless-monitor
   ```

2. **Install dependencies**
   ```bash
   cargo build
   ```

3. **Run tests**
   ```bash
   cargo test
   ```

4. **Run the application**
   ```bash
   cargo run -- server
   ```

## 🛠️ Development Guidelines

### Code Style

- Follow Rust conventions and use `cargo fmt` to format code
- Use `cargo clippy` to check for linting issues
- Write meaningful commit messages
- Add tests for new features

### Project Structure

```
src/
├── api/           # REST API endpoints
├── cli.rs         # Command-line interface
├── config.rs      # Configuration management
├── models.rs      # Data models and structures
├── monitoring.rs  # Core monitoring logic
└── ssh.rs         # SSH connection handling
```

### Testing

- Unit tests should be in the same file as the code they test
- Integration tests should be in the `tests/` directory
- Test SSH connections with local or test servers
- Ensure all tests pass before submitting PR

## 📝 Submitting Changes

### Pull Request Process

1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Write code following the style guidelines
   - Add tests for new functionality
   - Update documentation if needed

3. **Test your changes**
   ```bash
   cargo test
   cargo clippy
   cargo fmt -- --check
   ```

4. **Commit your changes**
   ```bash
   git add .
   git commit -m "feat: add your feature description"
   ```

5. **Push and create PR**
   ```bash
   git push origin feature/your-feature-name
   ```
   Then create a pull request on GitHub.

### Commit Message Format

Use conventional commits format:
- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation changes
- `style:` for formatting changes
- `refactor:` for code refactoring
- `test:` for adding tests
- `chore:` for maintenance tasks

Examples:
- `feat: add disk usage monitoring`
- `fix: resolve SSH connection timeout issue`
- `docs: update API documentation`

## 🐛 Reporting Issues

When reporting issues, please include:

1. **Environment information**
   - Operating system and version
   - Rust version (`rustc --version`)
   - Application version

2. **Steps to reproduce**
   - Clear, numbered steps
   - Expected vs actual behavior

3. **Logs and error messages**
   - Relevant log output
   - Full error messages

4. **Additional context**
   - Screenshots if applicable
   - Related issues or discussions

## 💡 Feature Requests

When suggesting features:

1. **Check existing issues** - Your idea might already be requested
2. **Describe the problem** - What problem does this solve?
3. **Propose a solution** - How should it work?
4. **Consider alternatives** - Are there other ways to solve this?

## 🏗️ Architecture Overview

### Core Components

- **SSH Manager** (`ssh.rs`) - Handles SSH connections and command execution
- **Monitoring Service** (`monitoring.rs`) - Collects and processes server metrics
- **API Layer** (`api/`) - REST endpoints for the web interface
- **Data Models** (`models.rs`) - Data structures and state management
- **Configuration** (`config.rs`) - Application configuration and settings

### Key Design Principles

- **Agentless** - No software installation on monitored servers
- **Efficient** - Minimal resource usage and fast performance
- **Secure** - SSH-based authentication and encrypted connections
- **Extensible** - Modular design for easy feature additions

## 📋 Development Roadmap

See the [GitHub Issues](https://github.com/tayyebi/agentless-monitoring/issues) for current priorities and planned features.

## 🤝 Community Guidelines

- Be respectful and inclusive
- Help others learn and grow
- Provide constructive feedback
- Follow the [Code of Conduct](CODE_OF_CONDUCT.md)

## 📞 Getting Help

- **GitHub Issues** - For bugs and feature requests
- **Discussions** - For questions and general discussion
- **Email** - For security issues (see SECURITY.md)

## 📄 License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to Agentless Monitor! 🎉
