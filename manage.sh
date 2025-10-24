#!/bin/bash

# Agentless Monitor Management Script (with environment install)
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="agentless-monitor"
BINARY_PATH="./target/release/$APP_NAME"
CONFIG_FILE="config.json"
SCRIPT_NAME="$(basename "$0")"

# Functions
print_usage() {
    echo "Usage: $0 {install|build|run|dev|clean|setup|status|logs|stop|test}"
    echo ""
    echo "Commands:"
    echo "  install    - Install toolchain and system dependencies (Linux/macOS)"
    echo "  build      - Build the application in release mode"
    echo "  run        - Run the application in release mode"
    echo "  dev        - Run the application in development mode"
    echo "  clean      - Clean build artifacts and config"
    echo "  setup      - Initial setup (create config file)"
    echo "  status     - Show application status"
    echo "  logs       - Show application logs"
    echo "  stop       - Stop the running application"
    echo "  test       - Run tests and check API endpoints"
    echo ""
}

print_status() {
    echo -e "${BLUE}=== Agentless Monitor Status ===${NC}"
    
    # Check if binary exists
    if [ -f "$BINARY_PATH" ]; then
        echo -e "Binary: ${GREEN}✓${NC} $BINARY_PATH"
    else
        echo -e "Binary: ${RED}✗${NC} Not found"
    fi
    
    # Check if config exists
    if [ -f "$CONFIG_FILE" ]; then
        echo -e "Config: ${GREEN}✓${NC} $CONFIG_FILE"
    else
        echo -e "Config: ${YELLOW}⚠${NC} Not found (will use defaults)"
    fi
    
    # Check if process is running
    if pgrep -f "$APP_NAME" > /dev/null; then
        echo -e "Process: ${GREEN}✓${NC} Running (PID: $(pgrep -f $APP_NAME))"
        echo -e "Web UI: ${GREEN}✓${NC} http://localhost:8080"
    else
        echo -e "Process: ${RED}✗${NC} Not running"
    fi
    
    echo ""
}

install_env() {
    echo -e "${BLUE}Installing environment and dependencies...${NC}"

    OS=""
    if [ "$(uname)" = "Darwin" ]; then
        OS="macos"
    elif [ -f /etc/os-release ]; then
        . /etc/os-release
        case "$ID" in
            ubuntu|debian) OS="debian" ;;
            centos|rhel|fedora) OS="rhel" ;;
            arch) OS="arch" ;;
            alpine) OS="alpine" ;;
            *) OS="linux" ;;
        esac
    else
        OS="linux"
    fi

    echo -e "Detected platform: ${YELLOW}$OS${NC}"

    # Helper: run package manager commands with sudo if not root
    SUDO=""
    if [ "$EUID" -ne 0 ]; then
        if command -v sudo &> /dev/null; then
            SUDO="sudo"
        else
            echo -e "${RED}sudo not found and not running as root. Please run as root or install sudo.${NC}"
            exit 1
        fi
    fi

    case "$OS" in
        debian)
            echo -e "${BLUE}Updating apt and installing packages...${NC}"
            $SUDO apt update
            $SUDO apt install -y build-essential curl pkg-config libssl-dev ca-certificates
            ;;
        rhel)
            echo -e "${BLUE}Installing packages with dnf/yum...${NC}"
            if command -v dnf &> /dev/null; then
                $SUDO dnf install -y gcc gcc-c++ make curl openssl-devel pkgconfig ca-certificates
            else
                $SUDO yum install -y gcc gcc-c++ make curl openssl-devel pkgconfig ca-certificates
            fi
            ;;
        arch)
            echo -e "${BLUE}Installing packages with pacman...${NC}"
            $SUDO pacman -Sy --noconfirm base-devel curl openssl ca-certificates
            ;;
        alpine)
            echo -e "${BLUE}Installing packages with apk...${NC}"
            $SUDO apk add --no-cache build-base curl openssl-dev ca-certificates
            ;;
        macos)
            echo -e "${BLUE}Installing packages with Homebrew (if available)...${NC}"
            if ! command -v brew &> /dev/null; then
                echo -e "${YELLOW}Homebrew not found. Installing Homebrew...${NC}"
                /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
                eval "$(/opt/homebrew/bin/brew shellenv 2>/dev/null || /usr/local/bin/brew shellenv 2>/dev/null || true)"
            fi
            brew install rustup-init openssl pkg-config curl
            ;;
        linux)
            echo -e "${YELLOW}Generic Linux detected. Please ensure curl, build tools and OpenSSL dev packages are installed.${NC}"
            ;;
        *)
            echo -e "${YELLOW}Unknown platform. Please install Rust toolchain, curl, build-essential and OpenSSL development headers manually.${NC}"
            ;;
    esac

    # Install rustup and toolchain if cargo not present
    if ! command -v cargo &> /dev/null; then
        echo -e "${BLUE}Installing Rust toolchain via rustup...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck disable=SC2016
        export PATH="$HOME/.cargo/bin:$PATH"
        echo -e "${GREEN}Rust toolchain installed. Ensure \$HOME/.cargo/bin is on your PATH.${NC}"
    else
        echo -e "${GREEN}Rust/Cargo already installed${NC}"
    fi

    echo -e "${GREEN}Environment install complete.${NC}"
    echo -e "${BLUE}Next steps: run './${SCRIPT_NAME} build' to compile the application${NC}"
    echo ""
}

setup() {
    echo -e "${BLUE}Setting up Agentless Monitor...${NC}"
    
    # Create config file if it doesn't exist
    if [ ! -f "$CONFIG_FILE" ]; then
        cat > "$CONFIG_FILE" << EOF
{
  "server_port": 8080,
  "log_level": "info",
  "monitoring_interval": 30,
  "ping_timeout": 5,
  "ssh_timeout": 10,
  "fallback_password": null
}
EOF
        echo -e "Created config file: ${GREEN}$CONFIG_FILE${NC}"
        echo -e "${YELLOW}You can edit $CONFIG_FILE to configure your settings${NC}"
    else
        echo -e "Config file already exists: ${GREEN}$CONFIG_FILE${NC}"
    fi
    
    # Set permissions on this script
    chmod +x "$SCRIPT_NAME"
    echo -e "Set executable permissions on $SCRIPT_NAME"
    
    echo -e "${GREEN}Setup complete!${NC}"
    echo -e "${BLUE}Note: This application uses in-memory storage.${NC}"
    echo -e "${BLUE}All data will be lost when the application stops.${NC}"
    echo ""
    print_status
}

build() {
    echo -e "${BLUE}Building $APP_NAME...${NC}"
    
    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: Rust/Cargo not found. Please run './$SCRIPT_NAME install' to install the toolchain.${NC}"
        exit 1
    fi
    
    # Build in release mode
    cargo build --release
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}Build successful!${NC}"
        echo -e "Binary location: ${GREEN}$BINARY_PATH${NC}"
    else
        echo -e "${RED}Build failed!${NC}"
        exit 1
    fi
}

run() {
    echo -e "${BLUE}Starting $APP_NAME...${NC}"
    
    # Check if binary exists
    if [ ! -f "$BINARY_PATH" ]; then
        echo -e "${YELLOW}Binary not found. Building first...${NC}"
        build
    fi
    
    # Setup if needed
    if [ ! -f "$CONFIG_FILE" ]; then
        echo -e "${YELLOW}First time setup...${NC}"
        setup
    fi
    
    # Run the application
    echo -e "${GREEN}Starting server on http://localhost:8080${NC}"
    echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
    echo -e "${BLUE}Note: Data is stored in memory and will be lost when stopped${NC}"
    echo ""
    
    exec "$BINARY_PATH" server
}

dev() {
    echo -e "${BLUE}Starting $APP_NAME in development mode...${NC}"
    
    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: Rust/Cargo not found. Please run './$SCRIPT_NAME install' to install the toolchain.${NC}"
        exit 1
    fi
    
    # Setup if needed
    if [ ! -f "$CONFIG_FILE" ]; then
        echo -e "${YELLOW}First time setup...${NC}"
        setup
    fi
    
    # Run in development mode
    echo -e "${GREEN}Starting development server on http://localhost:8080${NC}"
    echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
    echo -e "${BLUE}Note: Data is stored in memory and will be lost when stopped${NC}"
    echo ""
    
    RUST_LOG=debug cargo run -- server
}

clean() {
    echo -e "${BLUE}Cleaning $APP_NAME...${NC}"
    
    # Clean cargo build artifacts
    if command -v cargo &> /dev/null; then
        cargo clean
        echo -e "Cleaned build artifacts"
    else
        echo -e "${YELLOW}cargo not found, skipping cargo clean${NC}"
    fi
    
    # Remove config file
    if [ -f "$CONFIG_FILE" ]; then
        rm -f "$CONFIG_FILE"
        echo -e "Removed config file: ${YELLOW}$CONFIG_FILE${NC}"
    fi
    
    echo -e "${GREEN}Clean complete!${NC}"
}

logs() {
    echo -e "${BLUE}Application logs:${NC}"
    
    if pgrep -f "$APP_NAME" > /dev/null; then
        # Try to get logs from journalctl if available
        if command -v journalctl &> /dev/null; then
            journalctl -f -u "$APP_NAME" 2>/dev/null || echo "No systemd logs found"
        else
            echo "Application is running but no log viewer available"
            echo "Check the terminal where you started the application"
        fi
    else
        echo -e "${RED}Application is not running${NC}"
    fi
}

stop() {
    echo -e "${BLUE}Stopping $APP_NAME...${NC}"
    
    if pgrep -f "$APP_NAME" > /dev/null; then
        pkill -f "$APP_NAME"
        echo -e "${GREEN}Application stopped${NC}"
    else
        echo -e "${YELLOW}Application is not running${NC}"
    fi
}

test() {
    echo -e "${BLUE}Testing $APP_NAME...${NC}"
    
    # Check if application is running
    if ! pgrep -f "$APP_NAME" > /dev/null; then
        echo -e "${YELLOW}Application is not running. Building and starting it in background...${NC}"
        if [ ! -f "$BINARY_PATH" ]; then
            build
        fi
        "$BINARY_PATH" server &
        sleep 3
    fi
    
    echo -e "${BLUE}Testing API endpoints...${NC}"
    
    # Test health endpoint
    echo -n "Health check: "
    if curl -s http://localhost:8080/api/health > /dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi
    
    # Test servers endpoint
    echo -n "Servers list: "
    if curl -s http://localhost:8080/api/servers > /dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi
    
    # Test web interface
    echo -n "Web interface: "
    if curl -s http://localhost:8080/ > /dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi
    
    echo -e "${GREEN}Testing complete!${NC}"
    echo -e "Web interface available at: ${BLUE}http://localhost:8080${NC}"
}

# Main script logic
case "${1:-}" in
    install)
        install_env
        ;;
    build)
        build
        ;;
    run)
        run
        ;;
    dev)
        dev
        ;;
    clean)
        clean
        ;;
    setup)
        setup
        ;;
    status)
        print_status
        ;;
    logs)
        logs
        ;;
    stop)
        stop
        ;;
    test)
        test
        ;;
    *)
        print_usage
        exit 1
        ;;
esac