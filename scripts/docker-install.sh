# ---
# ems_section: "10-deployment-ci-cd-enhancements"
# ems_subsection: "docker-install-unix"
# ems_type: "script"
# ems_scope: "operations"
# ems_description: "Helper to install Docker prerequisites on Unix hosts."
# ems_version: "v0.0.0-prealpha"
# ems_owner: "tbd"
# ---
#!/usr/bin/env bash
set -euo pipefail

IMAGE_NAME=${R_EMS_IMAGE:-r-emsd:latest}
CONTAINER_NAME=${R_EMS_CONTAINER:-r-emsd}
PROJECT_ROOT=$(cd "$(dirname "$0")/.." && pwd)
CONFIG_DIR="$PROJECT_ROOT/target/docker-config"
LOG_DIR="$PROJECT_ROOT/target/docker-logs"
SNAPSHOT_DIR="$PROJECT_ROOT/target/docker-snapshots"
UI_STATIC_DIR="/opt/r-ems/ui/setup-wizard/public"

if ! command -v docker >/dev/null 2>&1; then
  echo "error: docker is not installed or not on PATH" >&2
  exit 1
fi

echo "[docker-install] Building image $IMAGE_NAME"
docker build -t "$IMAGE_NAME" "$PROJECT_ROOT"

mkdir -p "$CONFIG_DIR" "$LOG_DIR" "$SNAPSHOT_DIR"
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
  cp "$PROJECT_ROOT/configs/docker.default.toml" "$CONFIG_DIR/config.toml"
fi
chmod 0777 "$LOG_DIR" "$SNAPSHOT_DIR"
chmod 0666 "$CONFIG_DIR"/config.toml

if ! grep -Eq '^\[api\]' "$CONFIG_DIR/config.toml"; then
  cat <<EOF >>"$CONFIG_DIR/config.toml"

[api]
enabled = true
listen = "0.0.0.0:8080"
static_dir = "$UI_STATIC_DIR"
EOF
fi

if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
  echo "[docker-install] Removing existing container $CONTAINER_NAME"
  docker rm -f "$CONTAINER_NAME" >/dev/null
fi

echo "[docker-install] Starting container $CONTAINER_NAME"
docker run -d \
  --name "$CONTAINER_NAME" \
  --restart unless-stopped \
  -e R_EMS_CONFIG_PATH=/data/config/config.toml \
  -e R_EMS_LICENSE_BYPASS=1 \
  -e R_EMS_LOG=debug \
  -p 8080:8080 \
  -p 9898:9898 \
  -v "$CONFIG_DIR:/data/config" \
  -v "$LOG_DIR:/data/logs" \
  -v "$SNAPSHOT_DIR:/data/snapshots" \
  "$IMAGE_NAME" "$@"

echo "[docker-install] Container $CONTAINER_NAME is running. View logs with:"
echo "  docker logs -f $CONTAINER_NAME"
echo "[docker-install] The setup wizard UI will be available after startup at:"
echo "  http://localhost:8080/"
echo "[docker-install] To restart the service later run:"
echo "  docker start $CONTAINER_NAME"