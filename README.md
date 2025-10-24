# 🚀 Agentless Monitor

<div align="center">

**The modern, lightweight server monitoring solution that connects via SSH without requiring any agents on your target machines.**

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![GitHub release](https://img.shields.io/github/release/tayyebi/agentless-monitoring.svg?style=for-the-badge)](https://github.com/tayyebi/agentless-monitoring/releases)

[📥 Download Latest Release](#-download) • [📖 Documentation](#-documentation) • [🐛 Report Bug](https://github.com/tayyebi/agentless-monitoring/issues) • [💡 Request Feature](https://github.com/tayyebi/agentless-monitoring/issues)

</div>

---

## ✨ Why Agentless Monitor?

**Stop installing agents on every server.** Agentless Monitor connects to your servers via SSH and provides comprehensive monitoring without any software installation on target machines.

### 🎯 Key Benefits

- **🔒 Zero Agent Installation** - Monitor any server with SSH access
- **⚡ Real-time Performance** - Built with Rust for maximum speed and efficiency  
- **🌐 Modern Web Interface** - Beautiful, responsive dashboard
- **📊 Comprehensive Metrics** - CPU, Memory, Disks, Network, Ports, and more
- **🔐 Secure Authentication** - Supports SSH keys and password authentication
- **📈 Historical Data** - Track performance trends over time
- **🔧 Easy Setup** - Works with your existing SSH configuration

---

## 🖼️ Screenshots

### Dashboard Overview
![Dashboard Overview](docs/Screenshot%20from%202025-10-25%2001-32-08.png)
*Clean, modern interface showing server status and key metrics at a glance*

### Server Details & Monitoring
![Server Details](docs/Screenshot%20from%202025-10-25%2001-32-21.png)
*Detailed server information with real-time monitoring data and historical trends*

### Advanced Monitoring Features
![Advanced Features](docs/Screenshot%20from%202025-10-25%2001-32-59.png)
*Comprehensive monitoring including network interfaces, port status, and system information*

---

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ (for building from source)
- SSH access to target servers
- Modern web browser

### Installation

#### Option 1: Download Pre-built Binary (Recommended)

1. Download the latest release for your platform from the [Releases page](https://github.com/tayyebi/agentless-monitoring/releases)
2. Extract and run:
   ```bash
   ./agentless-monitor server
   ```

#### Option 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/tayyebi/agentless-monitoring.git
cd agentless-monitor

# Build the project
cargo build --release

# Run the server
./target/release/agentless-monitor server
```

### Configuration

1. **SSH Setup**: Ensure your SSH config (`~/.ssh/config`) contains your server definitions:
   ```
   Host myserver
       HostName 192.168.1.100
       User admin
       Port 22
   ```

2. **Start Monitoring**: Open your browser to `http://localhost:8080`

3. **Add Servers**: The application automatically loads servers from your SSH config

---

## 📊 Features

### 🔍 Real-time Monitoring
- **CPU Usage** - Current usage, load averages, and core information
- **Memory Stats** - Total, used, free, and swap memory details
- **Disk Usage** - Per-disk utilization and filesystem information
- **Network Interfaces** - Traffic, errors, and IP address monitoring
- **Port Status** - Open ports and associated processes
- **System Info** - Hostname, OS, kernel, uptime, and architecture

### 🛡️ Security & Authentication
- **SSH Key Support** - Use your existing SSH key infrastructure
- **Password Authentication** - Secure password storage with optional persistence
- **Connection Pooling** - Efficient SSH connection management
- **No Agent Required** - Zero footprint on monitored servers

### 📈 Data & Analytics
- **Historical Tracking** - Store up to 1000 data points per server
- **Trend Analysis** - Visualize performance over time
- **Real-time Updates** - Configurable monitoring intervals
- **Export Capabilities** - Access data via REST API

### 🌐 Web Interface
- **Responsive Design** - Works on desktop, tablet, and mobile
- **Modern UI** - Clean, intuitive interface with dark/light themes
- **Real-time Updates** - Live data refresh without page reload
- **Interactive Charts** - Rich visualizations for all metrics

---

## 🔧 Configuration

### SSH Configuration

The application automatically reads from your SSH config file. Example configuration:

```ssh
# Production servers
Host prod-web-01
    HostName web01.production.com
    User deploy
    Port 22
    IdentityFile ~/.ssh/prod_key

Host prod-db-01
    HostName db01.production.com
    User postgres
    Port 5432
    ProxyJump prod-web-01

# Development servers
Host dev-server
    HostName 192.168.1.50
    User developer
    Port 22
```

### Environment Variables

```bash
# Optional: Custom SSH config path
export SSH_CONFIG_PATH="/path/to/ssh/config"

# Optional: Custom port
export MONITOR_PORT="8080"

# Optional: Fallback password for servers requiring password auth
export FALLBACK_PASSWORD="your-password"
```

---

## 📖 API Documentation

### REST Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/servers` | GET | List all servers |
| `/api/servers/{id}` | GET | Get server details |
| `/api/servers/{id}/status` | GET | Get server status |
| `/api/servers/{id}/details/{metric}` | GET | Get specific metric data |
| `/api/servers/{id}/history` | GET | Get historical data |
| `/api/health` | GET | Health check |

### Example API Usage

```bash
# Get all servers
curl http://localhost:8080/api/servers

# Get server status
curl http://localhost:8080/api/servers/prod-web-01/status

# Get CPU details
curl http://localhost:8080/api/servers/prod-web-01/details/cpu
```

---

## 🛠️ Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/tayyebi/agentless-monitoring.git
cd agentless-monitor

# Install dependencies
cargo build

# Run in development mode
cargo run -- server

# Run tests
cargo test

# Build optimized release
cargo build --release
```

### Project Structure

```
agentless-monitoring/
├── src/
│   ├── api/           # REST API endpoints
│   ├── cli.rs         # Command-line interface
│   ├── config.rs      # Configuration management
│   ├── models.rs      # Data models and structures
│   ├── monitoring.rs  # Core monitoring logic
│   └── ssh.rs         # SSH connection handling
├── static/            # Web assets (CSS, JS)
├── templates/         # HTML templates
└── docs/             # Documentation and screenshots
```

---

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make your changes and add tests
4. Run tests: `cargo test`
5. Commit changes: `git commit -m 'Add amazing feature'`
6. Push to branch: `git push origin feature/amazing-feature`
7. Open a Pull Request

---

## 📋 Roadmap

- [ ] **Alert System** - Email/Slack notifications for thresholds
- [ ] **Docker Support** - Containerized deployment options
- [ ] **Metrics Export** - Prometheus/Grafana integration
- [ ] **Multi-tenant** - Support for multiple users/teams
- [ ] **Mobile App** - Native mobile applications
- [ ] **Plugin System** - Custom monitoring plugins

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/) and [Axum](https://github.com/tokio-rs/axum)
- UI powered by modern web technologies
- Inspired by the need for simple, effective server monitoring

---

<div align="center">

**⭐ Star this repository if you find it useful!**

[Report Bug](https://github.com/tayyebi/agentless-monitoring/issues) • [Request Feature](https://github.com/tayyebi/agentless-monitoring/issues) • [View Documentation](https://github.com/tayyebi/agentless-monitoring/wiki)

Made with ❤️ by Tayyebi

</div>