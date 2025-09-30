#!/usr/bin/env sh
# ---------------------------------------------------------------------------
# Helper to launch the core docker compose stack from any working directory.
# Resolves the repository root relative to this script so the compose file
# can be loaded without relying on the caller's current directory.
# ---------------------------------------------------------------------------
set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
COMPOSE_FILE="${PROJECT_ROOT}/compose/profiles/profile.core.yml"

if [ ! -f "$COMPOSE_FILE" ]; then
  printf 'Compose profile not found at %s\n' "$COMPOSE_FILE" >&2
  exit 1
fi

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
  printf 'Neither docker compose, docker-compose, nor podman compose was found in PATH.\n' >&2
  exit 1
fi

if [ -n "$COMPOSE_SUB" ]; then
  exec "$COMPOSE_BIN" "$COMPOSE_SUB" -f "$COMPOSE_FILE" "$@"
else
  exec "$COMPOSE_BIN" -f "$COMPOSE_FILE" "$@"
fi
