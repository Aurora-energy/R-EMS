#!/usr/bin/env sh
# ---------------------------------------------------------------------------
# Setup helper for the R-EMS bootstrap repository.
#
# The script ensures every shell helper in the `scripts/` directory is marked
# executable, builds the primary Rust container image, and captures a log of
# the operations. A short set of next steps is printed for developers after
# successful completion so that newcomers know how to launch the service.
# ---------------------------------------------------------------------------

set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
LOG_FILE="${SCRIPT_DIR}/setup.log"
IMAGE_NAME="r-ems-supervisor"
DOCKERFILE_PATH="${PROJECT_ROOT}/services/supervisor/Dockerfile"
RUN_CORE_HELPER="${PROJECT_ROOT}/scripts/run_core_stack.sh"
CORE_STACK_ARGS="up -d --build"
HTTP_HEALTH_ENDPOINTS="http://127.0.0.1:8080/healthz http://127.0.0.1:8000/healthz"

# shellcheck source=./common.sh
. "${SCRIPT_DIR}/common.sh"

log_to_file() {
  level="$1"
  shift
  message="$*"

  case "$level" in
    INFO)
      log_info "$message"
      ;;
    WARN)
      log_warn "$message"
      ;;
    ERROR)
      log_error "$message"
      ;;
    *)
      printf '[%s] %s\n' "$level" "$message"
      ;;
  esac

  printf '[%s] %s\n' "$level" "$message" >> "$LOG_FILE"
}

cleanup() {
  status="$1"
  if [ "$status" != "0" ]; then
    log_to_file ERROR "Setup terminated unexpectedly. Review ${LOG_FILE} for details."
  fi
}

require_http_client() {
  if command -v curl >/dev/null 2>&1; then
    HTTP_CLIENT="curl"
    return 0
  fi

  if command -v wget >/dev/null 2>&1; then
    HTTP_CLIENT="wget"
    return 0
  fi

  log_to_file ERROR "Neither curl nor wget is available for health checks. Install one of them and re-run the setup."
  exit 1
}

check_port_available() {
  port="$1"

  if command -v python3 >/dev/null 2>&1; then
    if python3 - "$port" <<'PY' >/dev/null 2>&1; then
import socket
import sys

port = int(sys.argv[1])
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
try:
    sock.bind(("0.0.0.0", port))
except OSError:
    sys.exit(1)
finally:
    sock.close()
PY
      return 0
    else
      return 1
    fi
  fi

  if command -v python >/dev/null 2>&1; then
    if python - "$port" <<'PY' >/dev/null 2>&1; then
import socket
import sys

port = int(sys.argv[1])
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
try:
    sock.bind(("0.0.0.0", port))
except OSError:
    sys.exit(1)
finally:
    sock.close()
PY
      return 0
    else
      return 1
    fi
  fi

  if command -v ss >/dev/null 2>&1; then
    if ss -ltn | awk -v port="$port" 'NR > 1 {n=split($4, a, ":"); if (a[n] == port) exit 1} END {exit 0}'; then
      return 0
    else
      return 1
    fi
  fi

  if command -v netstat >/dev/null 2>&1; then
    if netstat -ltn | awk -v port="$port" 'NR > 2 {n=split($4, a, ":"); if (a[n] == port) exit 1} END {exit 0}'; then
      return 0
    else
      return 1
    fi
  fi

  if command -v nc >/dev/null 2>&1; then
    if nc -z 127.0.0.1 "$port" >/dev/null 2>&1; then
      return 1
    fi
    if nc -z ::1 "$port" >/dev/null 2>&1; then
      return 1
    fi
    return 0
  fi

  log_to_file WARN "Unable to confirm availability of port ${port}; required tooling is missing. Assuming it is free."
  return 0
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

  log_to_file ERROR "Endpoint ${endpoint} did not become reachable after $max_attempts attempts."
  return 1
}

: > "$LOG_FILE"
trap 'cleanup "$?"' EXIT

log_to_file INFO "Starting R-EMS setup process."
log_to_file INFO "Scripts directory: ${SCRIPT_DIR}"
log_to_file INFO "Logging progress to: ${LOG_FILE}"

for script in "${SCRIPT_DIR}"/*.sh; do
  [ -f "$script" ] || continue
  base="$(basename "$script")"
  case "$base" in
    common.sh|setup.sh)
      log_to_file INFO "Skipping helper ${base}."
      continue
      ;;
  esac

  if [ ! -x "$script" ]; then
    chmod +x "$script"
    log_to_file INFO "Marked ${base} as executable."
  else
    log_to_file INFO "${base} is already executable."
  fi

done

log_to_file INFO "All helper scripts are ready."

REQUIRED_PORTS="7100 8000 8080"
for port in $REQUIRED_PORTS; do
  if ! check_port_available "$port"; then
    log_to_file ERROR "Port ${port} is already in use. Free the port or stop the conflicting service before re-running the setup."
    exit 1
  fi
  log_to_file INFO "Port ${port} is available."
done

log_to_file INFO "All required ports are free."

if [ ! -f "$DOCKERFILE_PATH" ]; then
  log_to_file ERROR "Expected Dockerfile not found at ${DOCKERFILE_PATH}."
  exit 1
fi

if [ ! -x "$RUN_CORE_HELPER" ]; then
  log_to_file ERROR "Expected stack helper not found at ${RUN_CORE_HELPER}."
  exit 1
fi

if command -v docker >/dev/null 2>&1; then
  CONTAINER_ENGINE="docker"
elif command -v podman >/dev/null 2>&1; then
  CONTAINER_ENGINE="podman"
else
  log_to_file ERROR "Neither docker nor podman was found in PATH. Install a container engine and re-run the setup."
  exit 1
fi

log_to_file INFO "Using container engine: ${CONTAINER_ENGINE}."
log_to_file INFO "Building ${IMAGE_NAME}:latest from ${DOCKERFILE_PATH}."

if ${CONTAINER_ENGINE} build -f "$DOCKERFILE_PATH" -t "${IMAGE_NAME}:latest" "$PROJECT_ROOT" >> "$LOG_FILE" 2>&1; then
  log_to_file INFO "Container image ${IMAGE_NAME}:latest built successfully."
else
  log_to_file ERROR "Failed to build ${IMAGE_NAME}:latest. Inspect ${LOG_FILE} for the full build output."
  exit 1
fi

require_http_client

log_to_file INFO "Launching the core stack with: ${RUN_CORE_HELPER} ${CORE_STACK_ARGS}."
if ${RUN_CORE_HELPER} ${CORE_STACK_ARGS} >> "$LOG_FILE" 2>&1; then
  log_to_file INFO "Core stack is starting. Waiting for health endpoints to respond."
else
  log_to_file ERROR "Failed to launch the core stack. Review ${LOG_FILE} for compose output."
  exit 1
fi

for endpoint in $HTTP_HEALTH_ENDPOINTS; do
  if ! probe_endpoint "$endpoint"; then
    log_to_file ERROR "Core services did not become ready."
    exit 1
  fi
done

log_to_file INFO "All core service endpoints are reachable."
log_to_file INFO "Setup completed successfully."

RUN_CORE_COMMAND="${RUN_CORE_HELPER} ${CORE_STACK_ARGS}"
SUPERVISOR_RUN_COMMAND="${CONTAINER_ENGINE} run --rm -p 7100:7100 ${IMAGE_NAME}:latest"

log_to_file INFO "Next steps:"
log_to_file INFO "  1. Visit http://localhost:8080 for the GUI and http://localhost:8000 for the registry API."
log_to_file INFO "  2. Tail the stack logs with: ${RUN_CORE_HELPER} logs -f"
log_to_file INFO "  3. If you prefer to run just the supervisor stub, start it with: ${SUPERVISOR_RUN_COMMAND} and confirm the health check at http://127.0.0.1:7100/healthz."
log_to_file INFO "  4. Review ${LOG_FILE} if you need to audit the installation process."

trap - EXIT
log_to_file INFO "You can review this log anytime at ${LOG_FILE}."
