#!/bin/bash

# Agentless Monitor Management Script
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="agentless_monitor"
RELEASE_PATH="./_build/prod/rel/$APP_NAME/bin/$APP_NAME"
CONFIG_FILE="config.json"
SCRIPT_NAME="$(basename "$0")"

# Functions
print_usage() {
    echo "Usage: $0 {install|build|run|dev|clean|setup|status|logs|stop|test}"
    echo ""
    echo "Commands:"
    echo "  install    - Install Elixir/Erlang toolchain and system dependencies"
    echo "  build      - Build the application in release mode"
    echo "  run        - Run the application in release mode"
    echo "  dev        - Run the application in development mode (mix run)"
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

    if [ -f "$RELEASE_PATH" ]; then
        echo -e "Release:  ${GREEN}✓${NC} $RELEASE_PATH"
    else
        echo -e "Release:  ${RED}✗${NC} Not built yet (run './$SCRIPT_NAME build')"
    fi

    if [ -f "$CONFIG_FILE" ]; then
        echo -e "Config:   ${GREEN}✓${NC} $CONFIG_FILE"
    else
        echo -e "Config:   ${YELLOW}⚠${NC} Not found (will use defaults)"
    fi

    if pgrep -f "$APP_NAME" > /dev/null; then
        echo -e "Process:  ${GREEN}✓${NC} Running (PID: $(pgrep -f $APP_NAME | head -1))"
        echo -e "Web UI:   ${GREEN}✓${NC} http://localhost:8080"
    else
        echo -e "Process:  ${RED}✗${NC} Not running"
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

    SUDO=""
    if [ "$EUID" -ne 0 ]; then
        if command -v sudo &> /dev/null; then
            SUDO="sudo"
        else
            echo -e "${RED}sudo not found and not running as root.${NC}"
            exit 1
        fi
    fi

    case "$OS" in
        debian)
            $SUDO apt-get update
            $SUDO apt-get install -y build-essential curl wget erlang elixir openssh-client iputils-ping
            ;;
        rhel)
            if command -v dnf &> /dev/null; then
                $SUDO dnf install -y erlang elixir curl openssh-clients iputils
            else
                $SUDO yum install -y erlang elixir curl openssh-clients iputils
            fi
            ;;
        arch)
            $SUDO pacman -Sy --noconfirm erlang elixir curl openssh iputils
            ;;
        alpine)
            $SUDO apk add --no-cache erlang elixir curl openssh-client iputils
            ;;
        macos)
            if ! command -v brew &> /dev/null; then
                /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
            fi
            brew install elixir
            ;;
        *)
            echo -e "${YELLOW}Please install Erlang/OTP and Elixir manually: https://elixir-lang.org/install.html${NC}"
            ;;
    esac

    if command -v mix &> /dev/null; then
        mix local.hex --force
        mix local.rebar --force
        echo -e "${GREEN}Elixir toolchain ready.${NC}"
    else
        echo -e "${RED}mix not found. Please install Elixir manually.${NC}"
        exit 1
    fi

    echo -e "${GREEN}Environment install complete.${NC}"
    echo -e "${BLUE}Next steps: run './${SCRIPT_NAME} build' to compile the application${NC}"
    echo ""
}

setup() {
    echo -e "${BLUE}Setting up Agentless Monitor...${NC}"

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
    else
        echo -e "Config file already exists: ${GREEN}$CONFIG_FILE${NC}"
    fi

    chmod +x "$SCRIPT_NAME"
    echo -e "${GREEN}Setup complete!${NC}"
    echo ""
    print_status
}

build() {
    echo -e "${BLUE}Building $APP_NAME...${NC}"

    if ! command -v mix &> /dev/null; then
        echo -e "${RED}Error: mix not found. Run './$SCRIPT_NAME install' first.${NC}"
        exit 1
    fi

    mix deps.get --only prod
    MIX_ENV=prod mix compile
    MIX_ENV=prod mix release --overwrite

    echo -e "${GREEN}Build successful!${NC}"
    echo -e "Release location: ${GREEN}$RELEASE_PATH${NC}"
}

run() {
    echo -e "${BLUE}Starting $APP_NAME...${NC}"

    if [ ! -f "$RELEASE_PATH" ]; then
        echo -e "${YELLOW}Release not found. Building first...${NC}"
        build
    fi

    if [ ! -f "$CONFIG_FILE" ]; then
        setup
    fi

    echo -e "${GREEN}Starting server on http://localhost:8080${NC}"
    echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
    echo ""

    exec "$RELEASE_PATH" start
}

dev() {
    echo -e "${BLUE}Starting $APP_NAME in development mode...${NC}"

    if ! command -v mix &> /dev/null; then
        echo -e "${RED}Error: mix not found. Run './$SCRIPT_NAME install' first.${NC}"
        exit 1
    fi

    if [ ! -f "$CONFIG_FILE" ]; then
        setup
    fi

    mix deps.get
    echo -e "${GREEN}Starting development server on http://localhost:8080${NC}"
    echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
    echo ""

    mix run --no-halt
}

clean() {
    echo -e "${BLUE}Cleaning $APP_NAME...${NC}"

    if command -v mix &> /dev/null; then
        mix clean
        rm -rf _build deps
        echo -e "Cleaned build artifacts"
    fi

    if [ -f "$CONFIG_FILE" ]; then
        rm -f "$CONFIG_FILE"
        echo -e "Removed config file: ${YELLOW}$CONFIG_FILE${NC}"
    fi

    echo -e "${GREEN}Clean complete!${NC}"
}

logs() {
    echo -e "${BLUE}Application logs:${NC}"

    if pgrep -f "$APP_NAME" > /dev/null; then
        if command -v journalctl &> /dev/null; then
            journalctl -f -u "$APP_NAME" 2>/dev/null || echo "No systemd logs found"
        else
            echo "Application is running – check the terminal where it was started"
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

test_app() {
    echo -e "${BLUE}Testing $APP_NAME...${NC}"

    if ! pgrep -f "$APP_NAME" > /dev/null; then
        echo -e "${YELLOW}Application is not running. Building and starting in background...${NC}"
        if [ ! -f "$RELEASE_PATH" ]; then
            build
        fi
        "$RELEASE_PATH" start &
        sleep 3
    fi

    echo -e "${BLUE}Running mix test...${NC}"
    mix test || true

    echo -e "${BLUE}Testing API endpoints...${NC}"

    echo -n "Health check: "
    if curl -s http://localhost:8080/api/health > /dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi

    echo -n "Servers list: "
    if curl -s http://localhost:8080/api/servers > /dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi

    echo -n "Web interface: "
    if curl -s http://localhost:8080/ > /dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi

    echo -e "${GREEN}Testing complete!${NC}"
    echo -e "Web interface: ${BLUE}http://localhost:8080${NC}"
}

# Main
case "${1:-}" in
    install) install_env ;;
    build)   build ;;
    run)     run ;;
    dev)     dev ;;
    clean)   clean ;;
    setup)   setup ;;
    status)  print_status ;;
    logs)    logs ;;
    stop)    stop ;;
    test)    test_app ;;
    *)       print_usage; exit 1 ;;
esac