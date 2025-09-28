# ---
# ems_section: "10-deployment-ci-cd-enhancements"
# ems_subsection: "docker-entrypoint"
# ems_type: "script"
# ems_scope: "operations"
# ems_description: "Container entrypoint for orchestrating services."
# ems_version: "v0.0.0-prealpha"
# ems_owner: "tbd"
# ---
#!/usr/bin/env bash
set -euo pipefail

CONFIG_SOURCE=${R_EMS_CONFIG_SOURCE:-/opt/r-ems/configs/docker.default.toml}
CONFIG_TARGET=${R_EMS_CONFIG_PATH:-/data/config/config.toml}
LICENSE_BYPASS=${R_EMS_LICENSE_BYPASS:-1}
MODE_OVERRIDE=${R_EMS_MODE:-}

mkdir -p "$(dirname "$CONFIG_TARGET")" /data/logs /data/snapshots

if [ ! -f "$CONFIG_TARGET" ]; then
  echo "[entrypoint] provisioning config at $CONFIG_TARGET"
  cp "$CONFIG_SOURCE" "$CONFIG_TARGET"
fi

COMMAND="${1:-run}"
shift || true

COMMAND_ARGS=("$@")

set -- /usr/local/bin/r-emsd --config "$CONFIG_TARGET"

if [ -n "$MODE_OVERRIDE" ]; then
  set -- "$@" --mode "$MODE_OVERRIDE"
fi

if [ "$LICENSE_BYPASS" = "1" ]; then
  set -- "$@" --dev-allow-license-bypass
fi

set -- "$@" "$COMMAND"

for arg in "${COMMAND_ARGS[@]}"; do
  set -- "$@" "$arg"
done

echo "[entrypoint] starting r-emsd with: $*"
exec "$@"
