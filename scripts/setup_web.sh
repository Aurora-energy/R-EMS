#!/usr/bin/env sh
# ---------------------------------------------------------------------------
# Web services orchestration helper for the R-EMS bootstrap repository.
#
# This script is responsible solely for managing the GUI, Registry, and
# Supervisor containers defined in the core docker compose profile. It will:
#   * stop any running stack to ensure the latest code is used,
#   * start the services via docker compose,
#   * perform health checks on the exposed HTTP endpoints, and
#   * capture a detailed activity log in scripts/setup_web.log.
# ---------------------------------------------------------------------------

set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
LOG_FILE="${SCRIPT_DIR}/setup_web.log"
COMPOSE_FILE="${PROJECT_ROOT}/compose/profiles/profile.core.yml"
DEFAULT_COMMAND="restart"
HTTP_ENDPOINTS="http://127.0.0.1:8080/healthz http://127.0.0.1:8000/healthz http://127.0.0.1:7100/healthz"
REQUIRED_PORTS="8080 8000 7100"

# shellcheck source=./common.sh
. "${SCRIPT_DIR}/common.sh"

log_to_file() {
  level="$1"
  shift
  message="$*"

  case "$level" in
    INFO) log_info "$message" ;;
    WARN) log_warn "$message" ;;
    ERROR) log_error "$message" ;;
    *) printf '[%s] %s\n' "$level" "$message" ;;
  esac

  printf '[%s] %s\n' "$level" "$message" >> "$LOG_FILE"
}

cleanup() {
  status="$1"
  if [ "$status" != "0" ]; then
    log_to_file ERROR "Web setup terminated unexpectedly. Review ${LOG_FILE} for details."
  fi
}

record_failure() {
  FAILURES=$((FAILURES + 1))
}

check_port_available() {
  port="$1"

  if command -v python3 >/dev/null 2>&1; then
    if python3 -c 'import socket, sys
port = int(sys.argv[1])
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
try:
    sock.bind(("0.0.0.0", port))
except OSError:
    raise SystemExit(1)
finally:
    sock.close()
' "$port" >/dev/null 2>&1; then return 0; else return 1; fi
  fi

  if command -v python >/dev/null 2>&1; then
    if python -c 'import socket, sys
port = int(sys.argv[1])
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
try:
    sock.bind(("0.0.0.0", port))
except OSError:
    raise SystemExit(1)
finally:
    sock.close()
' "$port" >/dev/null 2>&1; then return 0; else return 1; fi
  fi

  if command -v ss >/dev/null 2>&1; then
    if ss -ltn | awk -v port="$port" 'NR > 1 {n=split($4,a,":"); if(a[n]==port) exit 1} END{exit 0}'; then return 0; else return 1; fi
  fi

  if command -v netstat >/dev/null 2>&1; then
    if netstat -ltn | awk -v port="$port" 'NR > 2 {n=split($4,a,":"); if(a[n]==port) exit 1} END{exit 0}'; then return 0; else return 1; fi
  fi

  if command -v nc >/dev/null 2>&1; then
    if nc -z 127.0.0.1 "$port" >/dev/null 2>&1; then return 1; fi
    if nc -z ::1 "$port" >/dev/null 2>&1; then return 1; fi
    return 0
  fi

  log_to_file WARN "Unable to confirm availability of port ${port}; required tooling is missing. Assuming it is free."
  return 0
}

require_http_client() {
  if command -v curl >/dev/null 2>&1; then
    HTTP_CLIENT="curl"
    log_to_file INFO "Using curl for HTTP health checks."
    return 0
  fi
  if command -v wget >/dev/null 2>&1; then
    HTTP_CLIENT="wget"
    log_to_file INFO "Using wget for HTTP health checks."
    return 0
  fi
  log_to_file ERROR "Neither curl nor wget is available for health checks. Install one of them and re-run the script."
  return 1
}

probe_endpoint() {
  endpoint="$1"
  attempts=0
  max_attempts=30
  while [ "$attempts" -lt "$max_attempts" ]; do
    attempts=$((attempts + 1))
    case "$HTTP_CLIENT" in
      curl)
        if curl --silent --fail --max-time 5 "$endpoint" >/dev/null 2>&1; then
          log_to_file INFO "Endpoint ${endpoint} is reachable."
          return 0
        fi
        ;;
      wget)
        if wget -q --timeout=5 --tries=1 "$endpoint" -O /dev/null; then
          log_to_file INFO "Endpoint ${endpoint} is reachable."
          return 0
        fi
        ;;
    esac
    sleep 2
  done
  log_to_file ERROR "Endpoint ${endpoint} did not become reachable after ${max_attempts} attempts."
  return 1
}

select_compose() {
  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    COMPOSE_BIN="docker"
    COMPOSE_SUB="compose"
  elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_BIN="docker-compose"
    COMPOSE_SUB=""
  elif command -v podman >/dev/null 2>&1 && podman compose version >/dev/null 2>&1; then
    COMPOSE_BIN="podman"
    COMPOSE_SUB="compose"
  else
    log_to_file ERROR "Neither docker compose, docker-compose, nor podman compose was found in PATH."
    return 1
  fi
  return 0
}

run_compose() {
  if [ -n "$COMPOSE_SUB" ]; then
    "$COMPOSE_BIN" "$COMPOSE_SUB" -f "$COMPOSE_FILE" "$@"
  else
    "$COMPOSE_BIN" -f "$COMPOSE_FILE" "$@"
  fi
}

stop_services() {
  if run_compose down --remove-orphans >> "$LOG_FILE" 2>&1; then
    log_to_file INFO "Stopped any running web services stack."
    return 0
  fi
  log_to_file WARN "Attempt to stop the web services stack failed. Check ${LOG_FILE} for details."
  return 1
}

start_services() {
  if run_compose up -d --build >> "$LOG_FILE" 2>&1; then
    log_to_file INFO "Web services stack is starting."
    return 0
  fi
  log_to_file ERROR "Failed to start the web services stack. Review ${LOG_FILE} for compose output."
  return 1
}

wait_for_health() {
  failures=0
  for endpoint in $HTTP_ENDPOINTS; do
    if ! probe_endpoint "$endpoint"; then
      failures=$((failures + 1))
    fi
  done
  if [ "$failures" -eq 0 ]; then
    log_to_file INFO "All web service endpoints are reachable."
    return 0
  fi
  log_to_file ERROR "One or more web service endpoints failed health checks."
  return 1
}

ensure_ports_free() {
  for port in $REQUIRED_PORTS; do
    if check_port_available "$port"; then
      log_to_file INFO "Port ${port} is available."
    else
      log_to_file ERROR "Port ${port} is still in use after stopping services. Resolve the conflict and re-run the script."
      return 1
    fi
  done
  return 0
}

command="${1:-$DEFAULT_COMMAND}"
FAILURES=0
HTTP_CLIENT=""
COMPOSE_BIN=""
COMPOSE_SUB=""

: > "$LOG_FILE"
trap 'cleanup "$?"' EXIT

log_to_file INFO "Starting web services orchestration with command: ${command}."
log_to_file INFO "Compose file: ${COMPOSE_FILE}"
log_to_file INFO "Logging progress to: ${LOG_FILE}"

if [ ! -f "$COMPOSE_FILE" ]; then
  log_to_file ERROR "Compose profile not found at ${COMPOSE_FILE}."
  exit 1
fi

case "$command" in
  restart|start|stop) ;;
  *)
    log_to_file ERROR "Unsupported command '${command}'. Use start, stop, or restart."
    exit 1
    ;;
esac

if ! select_compose; then
  exit 1
fi

if [ "$command" != "stop" ]; then
  if ! require_http_client; then
    exit 1
  fi
fi

if [ "$command" != "start" ]; then
  if ! stop_services; then
    record_failure
  fi
  # Give the container engine a moment to release resources.
  sleep 2
fi

if [ "$command" = "stop" ]; then
  if [ "$FAILURES" -eq 0 ]; then
    log_to_file INFO "Web services stopped successfully."
  else
    log_to_file WARN "Web services stop command completed with warnings."
  fi
  trap - EXIT
  log_to_file INFO "You can review this log anytime at ${LOG_FILE}."
  exit "${FAILURES}"
fi

if ! ensure_ports_free; then
  exit 1
fi

if ! start_services; then
  exit 1
fi

if ! wait_for_health; then
  exit 1
fi

log_to_file INFO "Web services are running with the latest code."

trap - EXIT
log_to_file INFO "You can review this log anytime at ${LOG_FILE}."
