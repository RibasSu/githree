#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/backend"
FRONTEND_DIR="$ROOT_DIR/frontend"
RUN_DIR="$ROOT_DIR/.run"
LOG_DIR="$ROOT_DIR/.logs"

BACKEND_PORT="${BACKEND_PORT:-3001}"
FRONTEND_PORT="${FRONTEND_PORT:-5173}"

BACKEND_PID_FILE="$RUN_DIR/backend.pid"
FRONTEND_PID_FILE="$RUN_DIR/frontend.pid"

usage() {
  cat <<'EOF'
Usage: ./scripts/dev-services.sh [start|stop|restart|status|test|coverage]

Environment overrides:
  BACKEND_PORT   Backend listen port (default: 3001)
  FRONTEND_PORT  Frontend listen port (default: 5173)
  COVERAGE_DIR   Coverage output directory (default: ./coverage)
EOF
}

ensure_requirements() {
  command -v lsof >/dev/null 2>&1 || {
    echo "Missing dependency: lsof" >&2
    exit 1
  }
}

listening_pids_for_port() {
  local port="$1"
  local pids
  pids="$(lsof -ti "tcp:${port}" -sTCP:LISTEN 2>/dev/null || true)"
  if [[ -n "$pids" ]]; then
    printf '%s\n' "$pids"
    return 0
  fi
  return 1
}

wait_for_port_state() {
  local port="$1"
  local expected="$2" # "up" or "down"
  local attempts=240
  local sleep_secs=0.25

  while (( attempts > 0 )); do
    if [[ "$expected" == "up" ]] && listening_pids_for_port "$port" >/dev/null; then
      return 0
    fi
    if [[ "$expected" == "down" ]] && ! listening_pids_for_port "$port" >/dev/null; then
      return 0
    fi
    sleep "$sleep_secs"
    ((attempts--))
  done

  return 1
}

kill_pid_file() {
  local pid_file="$1"
  if [[ ! -f "$pid_file" ]]; then
    return
  fi

  local pid
  pid="$(cat "$pid_file" 2>/dev/null || true)"
  rm -f "$pid_file"

  if [[ -z "$pid" ]]; then
    return
  fi

  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    sleep 0.3
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
}

kill_port() {
  local port="$1"
  local pids
  pids="$(listening_pids_for_port "$port" || true)"
  if [[ -z "$pids" ]]; then
    return
  fi

  while IFS= read -r pid; do
    [[ -z "$pid" ]] && continue
    kill "$pid" 2>/dev/null || true
  done <<<"$pids"

  sleep 0.3

  pids="$(listening_pids_for_port "$port" || true)"
  if [[ -n "$pids" ]]; then
    while IFS= read -r pid; do
      [[ -z "$pid" ]] && continue
      kill -9 "$pid" 2>/dev/null || true
    done <<<"$pids"
  fi
}

stop_services() {
  mkdir -p "$RUN_DIR"

  echo "Stopping backend and frontend..."

  kill_pid_file "$BACKEND_PID_FILE"
  kill_pid_file "$FRONTEND_PID_FILE"

  kill_port "$BACKEND_PORT"
  kill_port "$FRONTEND_PORT"

  wait_for_port_state "$BACKEND_PORT" down || true
  wait_for_port_state "$FRONTEND_PORT" down || true

  echo "Stopped."
}

start_backend() {
  local log_file="$LOG_DIR/backend.log"
  : > "$log_file"
  nohup bash -lc "cd '$BACKEND_DIR' && exec cargo run" >>"$log_file" 2>&1 < /dev/null &
  local pid=$!
  disown "$pid" 2>/dev/null || true
  echo "$pid" > "$BACKEND_PID_FILE"
}

start_frontend() {
  local log_file="$LOG_DIR/frontend.log"
  : > "$log_file"
  nohup bash -lc "cd '$FRONTEND_DIR' && exec bun run dev --host 0.0.0.0 --port '$FRONTEND_PORT'" >>"$log_file" 2>&1 < /dev/null &
  local pid=$!
  disown "$pid" 2>/dev/null || true
  echo "$pid" > "$FRONTEND_PID_FILE"
}

start_services() {
  mkdir -p "$RUN_DIR" "$LOG_DIR"

  echo "Starting backend on :${BACKEND_PORT}..."

  start_backend

  echo "Starting frontend on :${FRONTEND_PORT}..."
  start_frontend

  wait_for_port_state "$BACKEND_PORT" up || {
    echo "Backend failed to start on port ${BACKEND_PORT}. See ${LOG_DIR}/backend.log" >&2
    tail -n 30 "${LOG_DIR}/backend.log" >&2 || true
    exit 1
  }
  wait_for_port_state "$FRONTEND_PORT" up || {
    echo "Frontend failed to start on port ${FRONTEND_PORT}. See ${LOG_DIR}/frontend.log" >&2
    tail -n 30 "${LOG_DIR}/frontend.log" >&2 || true
    exit 1
  }

  echo "Started."
  status_services
}

status_services() {
  local backend_status="down"
  local frontend_status="down"

  if listening_pids_for_port "$BACKEND_PORT" >/dev/null; then
    backend_status="up"
  fi
  if listening_pids_for_port "$FRONTEND_PORT" >/dev/null; then
    frontend_status="up"
  fi

  echo "Backend (${BACKEND_PORT}): ${backend_status}"
  echo "Frontend (${FRONTEND_PORT}): ${frontend_status}"
}

run_tests() {
  echo "Running backend tests..."
  (
    cd "$BACKEND_DIR"
    cargo test --all-targets --all-features
  )

  echo "Running frontend checks..."
  (
    cd "$FRONTEND_DIR"
    bun run check
    bun run build
  )

  echo "All tests/checks passed."
}

run_coverage() {
  local coverage_dir="${COVERAGE_DIR:-$ROOT_DIR/coverage}"
  local html_dir="$coverage_dir/backend-html"
  local html_index="$html_dir/html/index.html"
  local lcov_file="$coverage_dir/backend.lcov"

  if ! command -v cargo >/dev/null 2>&1; then
    echo "Missing dependency: cargo" >&2
    exit 1
  fi

  if ! cargo llvm-cov --version >/dev/null 2>&1; then
    cat >&2 <<'EOF'
Missing dependency: cargo-llvm-cov
Install it with:
  cargo install cargo-llvm-cov
EOF
    exit 1
  fi

  mkdir -p "$coverage_dir"

  echo "Generating backend HTML coverage report..."
  (
    cd "$BACKEND_DIR"
    cargo llvm-cov clean --workspace
    cargo llvm-cov --workspace --all-features --ignore-run-fail --html --output-dir "$html_dir"
  )

  echo "Generating backend LCOV report..."
  (
    cd "$BACKEND_DIR"
    cargo llvm-cov --workspace --all-features --ignore-run-fail --lcov --output-path "$lcov_file"
  )

  echo "Coverage reports generated:"
  echo "  HTML: ${html_index}"
  echo "  LCOV: ${lcov_file}"
}

main() {
  ensure_requirements

  local action="${1:-restart}"
  case "$action" in
    start)
      start_services
      ;;
    stop)
      stop_services
      ;;
    restart)
      stop_services
      start_services
      ;;
    status)
      status_services
      ;;
    test)
      run_tests
      ;;
    coverage)
      run_coverage
      ;;
    -h|--help|help)
      usage
      ;;
    *)
      echo "Unknown action: $action" >&2
      usage
      exit 1
      ;;
  esac
}

main "$@"
