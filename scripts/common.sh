#!/usr/bin/env sh
# ---------------------------------------------------------------------------
# Shared helper functions for installation scripts.
# This bootstrap version defines lightweight logging helpers to illustrate the
# structure. Future phases will expand the logic significantly.
# ---------------------------------------------------------------------------

log_info() {
  printf '[INFO] %s\n' "$1"
}

log_warn() {
  printf '[WARN] %s\n' "$1" >&2
}

log_error() {
  printf '[ERROR] %s\n' "$1" >&2
}
