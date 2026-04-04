#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
RUN_DIR="$ROOT_DIR/.run"
LOG_DIR="$ROOT_DIR/.logs"
INSTALL_DIR="$RUN_DIR/install"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
LOG_FILE="$LOG_DIR/install-${TIMESTAMP}.log"
HAS_TTY=0
USE_COLOR=0
STEP_INDEX=0
DOCKER_NEEDS_SUDO=0

if [[ -t 1 && -w /dev/tty ]]; then
  HAS_TTY=1
fi

mkdir -p "$RUN_DIR" "$LOG_DIR" "$INSTALL_DIR"
exec > >(tee -a "$LOG_FILE") 2>&1

if [[ $HAS_TTY -eq 1 && -z "${NO_COLOR:-}" ]]; then
  USE_COLOR=1
fi

if [[ $USE_COLOR -eq 1 ]]; then
  C_RESET=$'\033[0m'
  C_MUTED=$'\033[38;5;245m'
  C_INFO=$'\033[38;2;88;166;255m'
  C_WARN=$'\033[38;2;240;180;60m'
  C_ERROR=$'\033[38;2;255;95;95m'
  C_SUCCESS=$'\033[38;2;64;210;120m'
  C_PRIMARY=$'\033[38;2;240;80;50m'
  C_BOLD=$'\033[1m'
else
  C_RESET=""
  C_MUTED=""
  C_INFO=""
  C_WARN=""
  C_ERROR=""
  C_SUCCESS=""
  C_PRIMARY=""
  C_BOLD=""
fi

readonly ROOT_DIR RUN_DIR LOG_DIR INSTALL_DIR LOG_FILE HAS_TTY USE_COLOR

print_banner() {
  if [[ $USE_COLOR -eq 0 ]]; then
    return
  fi

  cat <<EOF
${C_PRIMARY}${C_BOLD}============================================================${C_RESET}
${C_PRIMARY}${C_BOLD}  GITHREE INSTALLER${C_RESET}
${C_MUTED}  Docker-first setup with guided checks and runtime wiring${C_RESET}
${C_PRIMARY}${C_BOLD}============================================================${C_RESET}
EOF
}

format_prompt() {
  local message="$1"
  if [[ $USE_COLOR -eq 1 ]]; then
    printf '%b%s%b' "$C_INFO$C_BOLD" "$message" "$C_RESET"
  else
    printf '%s' "$message"
  fi
}

on_error() {
  local exit_code="$?"
  local line_no="${1:-unknown}"
  printf '[%s] [ERROR] Installation failed at line %s: %s (exit=%s)\n' \
    "$(date +'%Y-%m-%d %H:%M:%S')" "$line_no" "${BASH_COMMAND:-unknown}" "$exit_code" >&2
  printf '[%s] [ERROR] Full log: %s\n' "$(date +'%Y-%m-%d %H:%M:%S')" "$LOG_FILE" >&2
  exit "$exit_code"
}
trap 'on_error $LINENO' ERR

log() {
  local level="$1"
  shift
  local color="$C_RESET"
  case "$level" in
    INFO) color="$C_INFO" ;;
    WARN) color="$C_WARN" ;;
    ERROR) color="$C_ERROR" ;;
    SUCCESS) color="$C_SUCCESS" ;;
    STEP) color="$C_PRIMARY" ;;
  esac

  printf '%b[%s] [%s] %s%b\n' "$color" "$(date +'%Y-%m-%d %H:%M:%S')" "$level" "$*" "$C_RESET"
}

info() { log INFO "$@"; }
warn() { log WARN "$@" >&2; }
die() { log ERROR "$@" >&2; exit 1; }
success() { log SUCCESS "$@"; }
step() {
  STEP_INDEX=$((STEP_INDEX + 1))
  log STEP "Step ${STEP_INDEX}: $*"
}

usage() {
  cat <<'EOF'
Usage: ./install.sh [--yes] [--help]

Options:
  --yes, -y    Non-interactive mode using defaults where possible
  --help, -h   Show this help
EOF
}

ASSUME_YES=0
for arg in "$@"; do
  case "$arg" in
    --yes|-y) ASSUME_YES=1 ;;
    --help|-h) usage; exit 0 ;;
    *) die "Unknown argument: $arg" ;;
  esac
done

SUDO_BIN=""
if [[ "$(id -u)" -ne 0 ]]; then
  if command -v sudo >/dev/null 2>&1; then
    SUDO_BIN="sudo"
  else
    die "This installer needs elevated privileges for package installation, but 'sudo' is not available."
  fi
fi

run() {
  info "Running: $*"
  "$@"
}

run_privileged() {
  if [[ -n "$SUDO_BIN" ]]; then
    run "$SUDO_BIN" "$@"
  else
    run "$@"
  fi
}

prompt() {
  local prompt_text="$1"
  local default_value="${2:-}"
  local result
  if [[ $ASSUME_YES -eq 1 ]]; then
    printf '%s\n' "$default_value"
    return 0
  fi
  if [[ -n "$default_value" ]]; then
    read -r -p "$(format_prompt "$prompt_text [$default_value]: ")" result
    printf '%s\n' "${result:-$default_value}"
  else
    read -r -p "$(format_prompt "$prompt_text: ")" result
    printf '%s\n' "$result"
  fi
}

prompt_yes_no() {
  local question="$1"
  local default_choice="${2:-yes}" # yes|no
  local input

  if [[ "$default_choice" != "yes" && "$default_choice" != "no" ]]; then
    die "Internal error: invalid default choice '$default_choice'"
  fi

  if [[ $ASSUME_YES -eq 1 ]]; then
    [[ "$default_choice" == "yes" ]]
    return
  fi

  local suffix="[y/N]"
  [[ "$default_choice" == "yes" ]] && suffix="[Y/n]"

  while true; do
    read -r -p "$(format_prompt "$question $suffix ")" input
    input="${input,,}"
    if [[ -z "$input" ]]; then
      [[ "$default_choice" == "yes" ]]
      return
    fi
    case "$input" in
      y|yes) return 0 ;;
      n|no) return 1 ;;
      *) warn "Please answer yes or no." ;;
    esac
  done
}

OS_NAME=""
PKG_MANAGER=""

detect_os() {
  case "$(uname -s)" in
    Linux) OS_NAME="linux" ;;
    Darwin) OS_NAME="macos" ;;
    *) die "Unsupported OS: $(uname -s). This installer supports Linux and macOS." ;;
  esac

  if [[ "$OS_NAME" == "linux" ]]; then
    if command -v apt-get >/dev/null 2>&1; then
      PKG_MANAGER="apt"
    elif command -v dnf >/dev/null 2>&1; then
      PKG_MANAGER="dnf"
    elif command -v pacman >/dev/null 2>&1; then
      PKG_MANAGER="pacman"
    elif command -v zypper >/dev/null 2>&1; then
      PKG_MANAGER="zypper"
    else
      PKG_MANAGER="unknown"
    fi
  else
    if command -v brew >/dev/null 2>&1; then
      PKG_MANAGER="brew"
    else
      PKG_MANAGER="unknown"
    fi
  fi

  info "Detected OS: $OS_NAME (package manager: $PKG_MANAGER)"
}

install_docker() {
  info "Attempting Docker installation for $OS_NAME ..."
  case "$PKG_MANAGER" in
    apt)
      run_privileged apt-get update
      run_privileged apt-get install -y docker.io docker-compose-plugin
      ;;
    dnf)
      run_privileged dnf install -y docker docker-compose-plugin
      ;;
    pacman)
      run_privileged pacman -Sy --noconfirm docker docker-compose
      ;;
    zypper)
      run_privileged zypper --non-interactive install docker docker-compose
      ;;
    brew)
      run brew install --cask docker
      warn "Docker Desktop was installed. Open Docker Desktop to start the daemon."
      ;;
    *)
      die "Automatic Docker installation is not supported on this OS/package manager. Install Docker manually and rerun."
      ;;
  esac
}

install_caddy() {
  info "Attempting host Caddy installation for $OS_NAME ..."
  case "$PKG_MANAGER" in
    apt)
      run_privileged apt-get update
      run_privileged apt-get install -y caddy
      ;;
    dnf)
      run_privileged dnf install -y caddy
      ;;
    pacman)
      run_privileged pacman -Sy --noconfirm caddy
      ;;
    zypper)
      run_privileged zypper --non-interactive install caddy
      ;;
    brew)
      run brew install caddy
      ;;
    *)
      die "Automatic Caddy installation is not supported on this OS/package manager."
      ;;
  esac
}

ensure_command() {
  local cmd="$1"
  local package_name="$2"
  local installer="$3"

  if command -v "$cmd" >/dev/null 2>&1; then
    info "Dependency found: $cmd"
    return 0
  fi

  warn "Missing dependency: $cmd"
  if prompt_yes_no "Install $package_name now?" "yes"; then
    "$installer"
  else
    die "Cannot continue without $cmd."
  fi

  command -v "$cmd" >/dev/null 2>&1 || die "$cmd is still unavailable after installation attempt."
}

COMPOSE_CMD=()

detect_compose() {
  if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
    info "Using Docker Compose command: docker compose"
    return
  fi
  if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
    info "Using Docker Compose command: docker-compose"
    return
  fi
  die "Docker Compose is not available. Install Docker Compose plugin and rerun."
}

can_access_docker_without_sudo() {
  docker info >/dev/null 2>&1
}

docker_access_error_output() {
  docker info 2>&1 >/dev/null || true
}

docker_access_denied_to_socket() {
  local err
  err="$(docker_access_error_output)"
  [[ "$err" == *"permission denied while trying to connect to the docker API"* ]]
}

docker_service_is_active() {
  if [[ "$OS_NAME" != "linux" ]]; then
    return 1
  fi
  systemctl is-active --quiet docker
}

configure_compose_privilege_mode() {
  if [[ $DOCKER_NEEDS_SUDO -ne 1 ]]; then
    return 0
  fi

  if [[ -z "$SUDO_BIN" ]]; then
    die "Docker requires privileged execution, but sudo is unavailable."
  fi

  COMPOSE_CMD=("$SUDO_BIN" "${COMPOSE_CMD[@]}")
  info "Docker commands for this installer run will use sudo."
}

ensure_docker_daemon() {
  if can_access_docker_without_sudo; then
    info "Docker daemon is running."
    return 0
  fi

  if docker_service_is_active && docker_access_denied_to_socket; then
    DOCKER_NEEDS_SUDO=1
    warn "Docker daemon is running, but current user cannot access /var/run/docker.sock."
    warn "This installer will continue using sudo for Docker commands."
    warn "Optional permanent fix: sudo usermod -aG docker \"${USER:-$(id -un)}\" && newgrp docker"
    return 0
  fi

  warn "Docker daemon is not running."
  local user_triggered_start="no"

  if [[ "$OS_NAME" == "linux" ]]; then
    if prompt_yes_no "Start Docker daemon now (systemctl enable --now docker)?" "yes"; then
      user_triggered_start="yes"
      run_privileged systemctl enable --now docker
    else
      die "Docker daemon is required, and startup was declined. Start Docker manually and rerun."
    fi
  else
    warn "On macOS, Docker Desktop must be running."
    if prompt_yes_no "Open Docker Desktop now?" "yes"; then
      user_triggered_start="yes"
      run open -a Docker
    else
      die "Docker daemon is required, and opening Docker Desktop was declined. Start it manually and rerun."
    fi
  fi

  if [[ "$user_triggered_start" != "yes" ]]; then
    die "Docker daemon is required but was not started."
  fi

  info "Waiting for Docker daemon to become available..."
  local attempts=60
  local spinner='|/-\'
  local spin_idx=0
  while (( attempts > 0 )); do
    if can_access_docker_without_sudo; then
      if [[ $HAS_TTY -eq 1 ]]; then
        printf '\r' > /dev/tty
      fi
      info "Docker daemon is now running."
      return 0
    fi

    if docker_service_is_active && docker_access_denied_to_socket; then
      DOCKER_NEEDS_SUDO=1
      if [[ $HAS_TTY -eq 1 ]]; then
        printf '\r' > /dev/tty
      fi
      warn "Docker daemon is running, but current user cannot access /var/run/docker.sock."
      warn "This installer will continue using sudo for Docker commands."
      warn "Optional permanent fix: sudo usermod -aG docker \"${USER:-$(id -un)}\" && newgrp docker"
      return 0
    fi

    if [[ $HAS_TTY -eq 1 ]]; then
      printf '\r%b[WAIT]%b Docker daemon startup in progress %s (%ss left)   ' \
        "$C_MUTED" "$C_RESET" "${spinner:spin_idx:1}" "$((attempts * 2))" > /dev/tty
      spin_idx=$(((spin_idx + 1) % 4))
    elif (( attempts % 10 == 0 )); then
      info "Still waiting for Docker daemon... (${attempts} checks remaining)"
    fi
    sleep 2
    ((attempts--))
  done

  if [[ $HAS_TTY -eq 1 ]]; then
    printf '\r' > /dev/tty
  fi
  die "Docker daemon is still not available. Start Docker manually and rerun."
}

check_port_free() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    if lsof -ti "tcp:${port}" -sTCP:LISTEN >/dev/null 2>&1; then
      return 1
    fi
  fi
  return 0
}

write_compose_file() {
  local app_port="$1"
  local rust_log="$2"
  local use_caddy="$3"
  local caddy_http_port="$4"
  local caddy_https_port="$5"

  local compose_file="$INSTALL_DIR/docker-compose.install.yml"
  local caddy_block=""
  local caddy_volume_block=""
  local caddy_ports_block=""

  if [[ "$use_caddy" == "yes" ]]; then
    caddy_block=$(cat <<EOF

  caddy:
    image: caddy:2.9-alpine
    container_name: githree-caddy
    depends_on:
      - githree
    restart: unless-stopped
    ports:
      - "${caddy_http_port}:80"
      - "${caddy_https_port}:443"
    volumes:
      - ${INSTALL_DIR}/Caddyfile:/etc/caddy/Caddyfile:ro
      - githree_caddy_data:/data
      - githree_caddy_config:/config
EOF
)
    caddy_volume_block=$(cat <<'EOF'
  githree_caddy_data:
  githree_caddy_config:
EOF
)
  fi

  cat >"$compose_file" <<EOF
services:
  githree:
    container_name: githree
    build:
      context: ${ROOT_DIR}
      dockerfile: ${ROOT_DIR}/Dockerfile
    restart: unless-stopped
    ports:
      - "${app_port}:3001"
    volumes:
      - githree_data:/app/data
      - ${ROOT_DIR}/config:/app/config:ro
    environment:
      - RUST_LOG=${rust_log}
${caddy_block}

volumes:
  githree_data:
${caddy_volume_block}
EOF

  info "Generated compose file: $compose_file"
}

write_caddy_file() {
  local domain="$1"
  local app_host_port="$2"
  local caddy_file="$INSTALL_DIR/Caddyfile"

  cat >"$caddy_file" <<EOF
${domain} {
  encode gzip
  reverse_proxy githree:3001
}
EOF
  info "Generated Caddyfile: $caddy_file"
  info "Caddy domain/host: $domain (upstream githree:3001, host app port ${app_host_port})"
}

main() {
  print_banner
  info "Starting Githree Docker installer"
  info "Repository root: $ROOT_DIR"
  info "Installer log: $LOG_FILE"

  step "Detecting host OS and package manager"
  detect_os

  step "Checking Docker installation"
  ensure_command docker "Docker" install_docker
  step "Checking Docker Compose availability"
  detect_compose
  step "Ensuring Docker daemon is ready"
  ensure_docker_daemon
  configure_compose_privilege_mode

  step "Collecting deployment settings"
  local app_port
  app_port="$(prompt "Host port for Githree web app" "3001")"
  [[ "$app_port" =~ ^[0-9]+$ ]] || die "Invalid app port: $app_port"
  check_port_free "$app_port" || die "Port $app_port is already in use. Choose another port."

  local rust_log
  rust_log="$(prompt "RUST_LOG level" "info")"

  local use_caddy="no"
  if prompt_yes_no "Enable Caddy reverse proxy container?" "no"; then
    use_caddy="yes"
  fi

  local caddy_domain=":80"
  local caddy_http_port="80"
  local caddy_https_port="443"

  if [[ "$use_caddy" == "yes" ]]; then
    caddy_domain="$(prompt "Caddy site address (example: githree.org or :80)" ":80")"
    caddy_http_port="$(prompt "Host HTTP port for Caddy" "80")"
    caddy_https_port="$(prompt "Host HTTPS port for Caddy" "443")"
    [[ "$caddy_http_port" =~ ^[0-9]+$ ]] || die "Invalid Caddy HTTP port: $caddy_http_port"
    [[ "$caddy_https_port" =~ ^[0-9]+$ ]] || die "Invalid Caddy HTTPS port: $caddy_https_port"
    check_port_free "$caddy_http_port" || die "Port $caddy_http_port is already in use."
    check_port_free "$caddy_https_port" || die "Port $caddy_https_port is already in use."

    if prompt_yes_no "Install Caddy on host too (optional)?" "no"; then
      install_caddy
    fi
    write_caddy_file "$caddy_domain" "$app_port"
  fi

  step "Generating compose and optional proxy configuration"
  write_compose_file "$app_port" "$rust_log" "$use_caddy" "$caddy_http_port" "$caddy_https_port"

  local compose_file="$INSTALL_DIR/docker-compose.install.yml"
  step "Building and starting containers"
  info "Deploying stack with Docker Compose ..."
  run "${COMPOSE_CMD[@]}" -f "$compose_file" up -d --build

  success "Deployment completed."
  success "App URL: http://localhost:${app_port}"
  if [[ "$use_caddy" == "yes" ]]; then
    info "Caddy enabled. Check routes with: ${COMPOSE_CMD[*]} -f $compose_file ps"
  fi
  info "Detailed installer log: $LOG_FILE"
}

main
