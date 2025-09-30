#!/usr/bin/env sh
# ---------------------------------------------------------------------------
# Configuration preparation helper for the R-EMS platform.
#
# This script validates the declarative system topology used by the configd
# service and ensures runtime log directories exist. All actions are written to
# scripts/setup_config.log for later review.
# ---------------------------------------------------------------------------

set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
LOG_FILE="${SCRIPT_DIR}/setup_config.log"
CONFIG_PATH="${PROJECT_ROOT}/configs/system.yaml"
RUNTIME_LOG_DIR="${PROJECT_ROOT}/logs"

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
    log_to_file ERROR "Configuration setup terminated unexpectedly. Review ${LOG_FILE} for details."
  fi
}

trap 'cleanup "$?"' EXIT

: > "$LOG_FILE"

log_to_file INFO "Starting configuration setup tasks."
log_to_file INFO "Scripts directory: ${SCRIPT_DIR}"
log_to_file INFO "Logging progress to: ${LOG_FILE}"

if [ ! -d "${PROJECT_ROOT}/configs" ]; then
  log_to_file INFO "Creating configuration directory at ${PROJECT_ROOT}/configs."
  mkdir -p "${PROJECT_ROOT}/configs"
fi

if [ ! -f "$CONFIG_PATH" ]; then
  TEMPLATE_PATH="${PROJECT_ROOT}/examples/configs/system.yaml"
  if [ -f "$TEMPLATE_PATH" ]; then
    cp "$TEMPLATE_PATH" "$CONFIG_PATH"
    log_to_file WARN "No system.yaml found. Copied template from ${TEMPLATE_PATH}."
  else
    log_to_file ERROR "Expected configuration file ${CONFIG_PATH} not found and no template available."
    exit 1
  fi
else
  log_to_file INFO "Found configuration file at ${CONFIG_PATH}."
fi

if [ ! -d "$RUNTIME_LOG_DIR" ]; then
  log_to_file INFO "Creating runtime log directory at ${RUNTIME_LOG_DIR}."
  mkdir -p "$RUNTIME_LOG_DIR"
else
  log_to_file INFO "Runtime log directory ${RUNTIME_LOG_DIR} already exists."
fi

log_to_file INFO "Validating configuration with r-ems-configd."
if cargo run -p r-ems-configd -- --config "$CONFIG_PATH" --log-dir "$RUNTIME_LOG_DIR" validate >> "$LOG_FILE" 2>&1; then
  log_to_file INFO "Configuration validated successfully."
else
  log_to_file ERROR "Configuration validation failed. Inspect ${LOG_FILE} for the detailed output."
  exit 1
fi

log_to_file INFO "Configuration setup completed."

trap - EXIT
log_to_file INFO "You can review this log anytime at ${LOG_FILE}."
