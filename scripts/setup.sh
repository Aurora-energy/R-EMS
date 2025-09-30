#!/usr/bin/env sh
# ---------------------------------------------------------------------------
# Core environment preparation for the R-EMS bootstrap repository.
#
# This script performs the minimal setup required before launching any web
# services:
#   * ensure helper scripts are executable,
#   * confirm the key service ports are available, and
#   * build the primary container image used by the supervisor.
# All actions are logged to scripts/setup.log to aid troubleshooting.
# ---------------------------------------------------------------------------

set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
LOG_FILE="${SCRIPT_DIR}/setup.log"
IMAGE_NAME="r-ems-supervisor"
DOCKERFILE_PATH="${PROJECT_ROOT}/services/supervisor/Dockerfile"

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
    log_to_file ERROR "Setup terminated unexpectedly. Review ${LOG_FILE} for details."
  fi
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

ensure_required_ports() {
  required_ports="7100 8000 8080"
  for port in $required_ports; do
    if check_port_available "$port"; then
      log_to_file INFO "Port ${port} is available."
    else
      log_to_file ERROR "Port ${port} is currently in use. Free the port before launching the web services."
      return 1
    fi
  done
  return 0
}

: > "$LOG_FILE"
trap 'cleanup "$?"' EXIT

log_to_file INFO "Starting core setup tasks."
log_to_file INFO "Scripts directory: ${SCRIPT_DIR}"
log_to_file INFO "Logging progress to: ${LOG_FILE}"

for script in "${SCRIPT_DIR}"/*.sh; do
  [ -f "$script" ] || continue
  base="$(basename "$script")"
  case "$base" in
    common.sh|setup.sh) log_to_file INFO "Skipping helper ${base}."; continue ;;
  esac
  if [ ! -x "$script" ]; then
    chmod +x "$script"
    log_to_file INFO "Marked ${base} as executable."
  else
    log_to_file INFO "${base} is already executable."
  fi
done

log_to_file INFO "All helper scripts are ready."

if [ ! -f "$DOCKERFILE_PATH" ]; then
  log_to_file ERROR "Expected Dockerfile not found at ${DOCKERFILE_PATH}."
  exit 1
fi

log_to_file INFO "Verifying required ports are free."
if ! ensure_required_ports; then
  log_to_file ERROR "One or more required ports are in use. Resolve the conflict and rerun the setup."
  exit 1
fi

CONTAINER_ENGINE=""
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

log_to_file INFO "Core setup completed successfully."
log_to_file INFO "Next step: run ./scripts/setup_web.sh to launch the web services stack."

trap - EXIT
log_to_file INFO "You can review this log anytime at ${LOG_FILE}."
