# Contributing to Agentless Monitor

Thank you for your interest in contributing to Agentless Monitor! This document provides guidelines and information for contributors.

## 🚀 Getting Started

### Prerequisites

- Elixir 1.14+ and Erlang/OTP 25+
- Git
- SSH access to test servers (for testing)

### Development Setup

1. **Fork and clone the repository**
   ```bash
   git clone https://github.com/tayyebi/agentless-monitoring.git
   cd agentless-monitoring
   ```

2. **Install dependencies**
   ```bash
   mix deps.get
   ```

3. **Run tests**
   ```bash
   mix test
   ```

4. **Run the application**
   ```bash
   mix run --no-halt
   ```

## 🛠️ Development Guidelines

### Code Style

- Follow Elixir conventions and use `mix format` to format code
- Use `mix credo` to check for linting issues (if configured)
- Write meaningful commit messages
- Add tests for new features

### Project Structure

```
lib/
└── agentless_monitor/
    ├── api/           # REST API endpoints
    ├── application.ex # OTP application entry point
    ├── config.ex      # Configuration management
    ├── models.ex      # Data models and structures
    ├── monitoring/    # Core monitoring logic
    ├── ssh/           # SSH connection handling
    └── state.ex       # GenServer state management
```

### Testing

- Unit tests live in the `test/` directory mirroring `lib/`
- Integration tests should cover SSH connections with local or test servers
- Ensure all tests pass before submitting a PR: `mix test`

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
   mix test
   mix format --check-formatted
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
   - Elixir version (`elixir --version`)
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

- **SSH Manager** (`ssh/`) - Handles SSH connections and command execution
- **Monitoring Service** (`monitoring/`) - Collects and processes server metrics
- **API Layer** (`api/`) - REST endpoints for the web interface
- **Data Models** (`models.ex`) - Data structures and state management
- **Configuration** (`config.ex`) - Application configuration and settings
- **State** (`state.ex`) - GenServer managing in-memory application state

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
