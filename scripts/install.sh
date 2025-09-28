# ---
# ems_section: "10-deployment-ci-cd-enhancements"
# ems_subsection: "installer-unix"
# ems_type: "script"
# ems_scope: "operations"
# ems_description: "Installer script for Unix-like hosts."
# ems_version: "v0.0.0-prealpha"
# ems_owner: "tbd"
# ---
#!/usr/bin/env bash
set -euo pipefail

LOG_PREFIX="[r-ems install]"
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
LOG_DIR="/var/log/r-ems"
INSTALL_LOG=""

INSTALL_DOCKER=1
RUN_CONTAINER=1
SYSTEMD_SETUP=1
INSTALL_RUST_MODE="auto"
STACK_ROOT="/opt/r-ems"
STATE_ROOT="/var/lib/r-ems"
CONFIG_ROOT="/etc/r-ems"
SYSTEMD_UNIT="r-emsd.service"
BUILDER_NAME="r-ems-builder"
IMAGE_NAME="ghcr.io/ocean-batteries/r-ems:local"
TARGET_PLATFORM="linux/amd64"
PROJECT_NAME="r-ems"
EXTRA_COMPOSE_ARGS=()
PLATFORM="unknown"
SUDO=""
DOCKER_CMD=()
APP_VERSION="dev"

usage() {
  cat <<'USAGE'
Usage: scripts/install.sh [options] [-- <docker compose args>]

Options:
  --skip-docker         Skip Docker Engine installation
  --skip-run            Do not start the runtime after provisioning
  --no-systemd          Do not install a systemd unit; run docker compose manually
  --install-rust        Force Rust toolchain installation via rustup (host builds)
  --no-rust             Never install the Rust toolchain
  --image <tag>         Tag to apply to the locally built image (default: r-emsd:local)
  --platform <target>   Target platform for docker buildx (default: linux/amd64)
  --builder <name>      Name of the docker buildx builder (default: r-ems-builder)
  --stack-root <dir>    Directory to store compose assets (default: /opt/r-ems)
  --state-root <dir>    Directory for runtime state such as logs/snapshots (default: /var/lib/r-ems)
  --config-root <dir>   Directory for configuration and env files (default: /etc/r-ems)
  --help                Show this message and exit

Arguments after `--` are forwarded to `docker compose up` when systemd is disabled.
USAGE
}

log() {
  echo "${LOG_PREFIX} $*"
}

error() {
  echo "${LOG_PREFIX} error: $*" >&2
  exit 1
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --skip-docker)
        INSTALL_DOCKER=0
        ;;
      --skip-run)
        RUN_CONTAINER=0
        ;;
      --no-systemd)
        SYSTEMD_SETUP=0
        ;;
      --install-rust)
        INSTALL_RUST_MODE="always"
        ;;
      --no-rust)
        INSTALL_RUST_MODE="never"
        ;;
      --image)
        shift || error "--image requires a tag"
        IMAGE_NAME="$1"
        ;;
      --image=*)
        IMAGE_NAME="${1#*=}"
        ;;
      --platform)
        shift || error "--platform requires a target"
        TARGET_PLATFORM="$1"
        ;;
      --platform=*)
        TARGET_PLATFORM="${1#*=}"
        ;;
      --builder)
        shift || error "--builder requires a name"
        BUILDER_NAME="$1"
        ;;
      --builder=*)
        BUILDER_NAME="${1#*=}"
        ;;
      --stack-root)
        shift || error "--stack-root requires a directory"
        STACK_ROOT="$1"
        ;;
      --stack-root=*)
        STACK_ROOT="${1#*=}"
        ;;
      --state-root)
        shift || error "--state-root requires a directory"
        STATE_ROOT="$1"
        ;;
      --state-root=*)
        STATE_ROOT="${1#*=}"
        ;;
      --config-root)
        shift || error "--config-root requires a directory"
        CONFIG_ROOT="$1"
        ;;
      --config-root=*)
        CONFIG_ROOT="${1#*=}"
        ;;
      --help)
        usage
        exit 0
        ;;
      --)
        shift
        EXTRA_COMPOSE_ARGS=("$@")
        break
        ;;
      *)
        error "unknown option: $1"
        ;;
    esac
    shift || true
  done
}

require_elevated_access() {
  if [[ $(id -u) -eq 0 ]]; then
    SUDO=""
  elif command_exists sudo; then
    SUDO="sudo"
  else
    error "sudo is required for package installation"
  fi
  if [[ -n $SUDO ]]; then
    DOCKER_CMD=($SUDO docker)
  else
    DOCKER_CMD=(docker)
  fi
}

setup_logging() {
  # Ensure installer output is captured in a well-known log file for
  # troubleshooting while still streaming to the console.
  local owner
  owner=${SUDO_USER:-$USER}
  INSTALL_LOG="$LOG_DIR/install.log"

  if [[ -n $SUDO ]]; then
    $SUDO mkdir -p "$LOG_DIR"
    $SUDO touch "$INSTALL_LOG"
    if [[ -n $owner ]]; then
      $SUDO chown "$owner" "$INSTALL_LOG"
    fi
  else
    mkdir -p "$LOG_DIR"
    touch "$INSTALL_LOG"
    if [[ -n $owner ]]; then
      chown "$owner" "$INSTALL_LOG" 2>/dev/null || true
    fi
  fi

  exec > >(tee -a "$INSTALL_LOG")
  exec 2>&1
  log "Streaming installer logs to $INSTALL_LOG"
}

detect_platform() {
  if [[ ! -f /etc/os-release ]]; then
    error "/etc/os-release not found; unsupported platform"
  fi
  # shellcheck disable=SC1091
  source /etc/os-release
  OS_ID=${ID:-}
  OS_ID_LIKE=${ID_LIKE:-}
  OS_VERSION_CODENAME=${VERSION_CODENAME:-}
  OS_VERSION_ID=${VERSION_ID:-}

  if [[ $OS_ID == "ubuntu" || $OS_ID == "debian" || $OS_ID_LIKE == *"debian"* ]]; then
    PLATFORM="apt"
  elif [[ $OS_ID == "fedora" || $OS_ID == "centos" || $OS_ID == "rhel" || $OS_ID_LIKE == *"fedora"* || $OS_ID_LIKE == *"rhel"* ]]; then
    if command_exists dnf; then
      PLATFORM="dnf"
    elif command_exists yum; then
      PLATFORM="yum"
    fi
  fi
}

ensure_prereq_tools() {
  if command_exists curl && command_exists git; then
    return
  fi
  case "$PLATFORM" in
    apt)
      log "Installing curl and git prerequisites (apt)"
      $SUDO apt-get update
      $SUDO apt-get install -y curl git
      ;;
    dnf)
      log "Installing curl and git prerequisites (dnf)"
      $SUDO dnf install -y curl git
      ;;
    yum)
      log "Installing curl and git prerequisites (yum)"
      $SUDO yum install -y curl git
      ;;
    *)
      log "curl or git missing; install manually"
      ;;
  esac
}

add_user_to_docker_group() {
  local user_to_modify
  user_to_modify=${SUDO_USER:-$USER}
  if [[ -z $user_to_modify ]]; then
    return
  fi
  if ! id "$user_to_modify" >/dev/null 2>&1; then
    return
  fi
  if id -nG "$user_to_modify" | grep -qw docker; then
    return
  fi
  if ! getent group docker >/dev/null 2>&1; then
    log "docker group not present; skipping group membership"
    return
  fi
  log "Adding $user_to_modify to docker group"
  $SUDO usermod -aG docker "$user_to_modify"
  log "User $user_to_modify added to docker group; a new login session is required for direct docker access"
}

start_docker_service() {
  if command_exists systemctl; then
    log "Enabling and starting docker service"
    $SUDO systemctl enable --now docker
  elif command_exists service; then
    log "Starting docker service"
    $SUDO service docker start
  else
    log "Unable to locate systemctl/service to start docker; start it manually if needed"
  fi
}

install_docker_with_apt() {
  log "Installing Docker Engine using apt"
  $SUDO apt-get update
  $SUDO apt-get install -y ca-certificates curl gnupg lsb-release
  $SUDO install -m 0755 -d /etc/apt/keyrings
  if [[ ! -f /etc/apt/keyrings/docker.gpg ]]; then
    curl -fsSL https://download.docker.com/linux/${OS_ID}/gpg | $SUDO gpg --dearmor -o /etc/apt/keyrings/docker.gpg
  fi
  $SUDO chmod a+r /etc/apt/keyrings/docker.gpg

  local arch codename repo
  arch=$(dpkg --print-architecture)
  codename=$OS_VERSION_CODENAME
  if [[ -z $codename ]] && command_exists lsb_release; then
    codename=$(lsb_release -cs)
  fi
  if [[ -z $codename ]]; then
    error "Unable to determine distribution codename for apt repository"
  fi
  repo="deb [arch=${arch} signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/${OS_ID} ${codename} stable"
  echo "$repo" | $SUDO tee /etc/apt/sources.list.d/docker.list >/dev/null

  $SUDO apt-get update
  $SUDO apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
  start_docker_service
}

install_docker_with_dnf() {
  log "Installing Docker Engine using dnf"
  $SUDO dnf -y install dnf-plugins-core
  $SUDO dnf config-manager --add-repo https://download.docker.com/linux/${OS_ID}/docker-ce.repo
  $SUDO dnf -y install docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
  start_docker_service
}

install_docker_with_yum() {
  log "Installing Docker Engine using yum"
  $SUDO yum install -y yum-utils
  $SUDO yum-config-manager --add-repo https://download.docker.com/linux/${OS_ID}/docker-ce.repo
  $SUDO yum install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
  start_docker_service
}

install_docker_engine() {
  if command_exists docker; then
    log "Docker already installed"
    start_docker_service
    return
  fi

  case "$PLATFORM" in
    apt)
      install_docker_with_apt
      ;;
    dnf)
      install_docker_with_dnf
      ;;
    yum)
      install_docker_with_yum
      ;;
    *)
      error "Unsupported distribution: ${OS_ID}. Install Docker manually and rerun with --skip-docker"
      ;;
  esac
}

should_install_rust() {
  case "$INSTALL_RUST_MODE" in
    always)
      return 0
      ;;
    never)
      return 1
      ;;
    auto)
      if command_exists cargo; then
        return 1
      fi
      ;;
  esac
  return 0
}

install_rust_toolchain() {
  if ! should_install_rust; then
    return
  fi
  log "Installing Rust toolchain via rustup"
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
  export PATH="$HOME/.cargo/bin:$PATH"
}

verify_docker_access() {
  if ! command_exists docker; then
    error "docker command not found after installation"
  fi
  if ! "${DOCKER_CMD[@]}" info >/dev/null 2>&1; then
    log "docker info failed; if you were added to the docker group, log out and back in or rerun the script with sudo"
  fi
}

ensure_compose_plugin() {
  if ! "${DOCKER_CMD[@]}" compose version >/dev/null 2>&1; then
    error "docker compose plugin not available; install docker-compose-plugin package"
  fi
}

ensure_buildx_builder() {
  if ! "${DOCKER_CMD[@]}" buildx version >/dev/null 2>&1; then
    error "docker buildx plugin not available; install docker-buildx-plugin package"
  fi
  if ! "${DOCKER_CMD[@]}" buildx ls | grep -qw "$BUILDER_NAME"; then
    log "Creating docker buildx builder $BUILDER_NAME"
    "${DOCKER_CMD[@]}" buildx create --name "$BUILDER_NAME" --driver docker-container >/dev/null
  fi
  "${DOCKER_CMD[@]}" buildx use "$BUILDER_NAME" >/dev/null
  "${DOCKER_CMD[@]}" buildx inspect "$BUILDER_NAME" --bootstrap >/dev/null
}

determine_app_version() {
  if [[ -n ${R_EMS_VERSION:-} ]]; then
    APP_VERSION="$R_EMS_VERSION"
    return
  fi
  local detected
  detected=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | awk -F'"' '{print $2}') || true
  APP_VERSION=${detected:-dev}
}

build_container_image() {
  log "Building container image ${IMAGE_NAME} for ${TARGET_PLATFORM}"
  "${DOCKER_CMD[@]}" buildx build \
    --builder "$BUILDER_NAME" \
    --platform "$TARGET_PLATFORM" \
    --build-arg APP_VERSION="$APP_VERSION" \
    -t "$IMAGE_NAME" \
    --load \
    "$PROJECT_ROOT"
}

stage_runtime_stack() {
  local stack_dir config_dir log_dir snapshot_dir compose_src env_target
  stack_dir="$STACK_ROOT/runtime"
  config_dir="$CONFIG_ROOT"
  log_dir="$STATE_ROOT/logs"
  snapshot_dir="$STATE_ROOT/snapshots"
  compose_src="$PROJECT_ROOT/configs/deploy/docker-compose.prod.yml"
  env_target="$stack_dir/.env"

  log "Staging runtime assets under $stack_dir"
  $SUDO mkdir -p "$stack_dir" "$config_dir" "$log_dir" "$snapshot_dir"
  $SUDO chmod 0755 "$config_dir"
  if [[ ! -f $compose_src ]]; then
    error "docker-compose definition not found at $compose_src"
  fi
  $SUDO install -m 0644 "$compose_src" "$stack_dir/docker-compose.yml"

  if [[ ! -f "$config_dir/config.toml" ]]; then
    $SUDO install -m 0644 "$PROJECT_ROOT/configs/docker.default.toml" "$config_dir/config.toml"
  fi

  if getent group docker >/dev/null 2>&1; then
    $SUDO chgrp docker "$log_dir" "$snapshot_dir" || true
  fi
  $SUDO chmod 0775 "$log_dir" "$snapshot_dir"

  cat <<ENV | $SUDO tee "$env_target" >/dev/null
R_EMS_IMAGE=$IMAGE_NAME
R_EMS_CONTAINER=r-emsd
R_EMS_CONFIG_DIR=$config_dir
R_EMS_DATA_DIR=$STATE_ROOT
R_EMS_LOG_DIR=$log_dir
R_EMS_SNAPSHOT_DIR=$snapshot_dir
R_EMS_CONFIG_SOURCE=/opt/r-ems/configs/docker.default.toml
R_EMS_VERSION=$APP_VERSION
R_EMS_LICENSE_BYPASS=1
R_EMS_LOG_LEVEL=info
R_EMS_METRICS_PORT=9898
R_EMS_API_PORT=8080
ENV
}

stage_native_units() {
  local unit_dir
  unit_dir="$PROJECT_ROOT/configs/deploy/systemd"
  if [[ ! -d $unit_dir ]]; then
    return
  fi
  log "Copying native systemd units to /etc/systemd/system for optional bare-metal installs"
  for unit in "$unit_dir"/*.service; do
    [[ -e $unit ]] || continue
    $SUDO install -m 0644 "$unit" "/etc/systemd/system/$(basename "$unit")"
  done
}

install_systemd_unit() {
  if [[ $SYSTEMD_SETUP -eq 0 ]]; then
    log "Skipping systemd unit installation (--no-systemd)"
    return
  fi
  if ! command_exists systemctl; then
    log "systemctl not available; falling back to docker compose"
    SYSTEMD_SETUP=0
    return
  fi

  local stack_dir docker_bin
  stack_dir="$STACK_ROOT/runtime"
  docker_bin=$(command -v docker)
  log "Installing systemd unit $SYSTEMD_UNIT"
  cat <<UNIT | $SUDO tee /etc/systemd/system/$SYSTEMD_UNIT >/dev/null
[Unit]
Description=R-EMS Orchestrator Container
After=network-online.target docker.service
Wants=network-online.target
Requires=docker.service

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=$stack_dir
ExecStart=$docker_bin compose --project-name $PROJECT_NAME --file docker-compose.yml up -d
ExecStop=$docker_bin compose --project-name $PROJECT_NAME --file docker-compose.yml down
ExecReload=$docker_bin compose --project-name $PROJECT_NAME --file docker-compose.yml up -d
TimeoutStopSec=90

[Install]
WantedBy=multi-user.target
UNIT
}

start_runtime() {
  local stack_dir
  stack_dir="$STACK_ROOT/runtime"

  if [[ $RUN_CONTAINER -eq 0 ]]; then
    log "Skipping runtime start (--skip-run)"
    return
  fi

  if [[ $SYSTEMD_SETUP -eq 1 ]]; then
    log "Enabling and starting $SYSTEMD_UNIT"
    $SUDO systemctl daemon-reload
    $SUDO systemctl enable --now "$SYSTEMD_UNIT"
  else
    log "Launching stack with docker compose"
    (cd "$stack_dir" && "${DOCKER_CMD[@]}" compose --project-name "$PROJECT_NAME" up -d "${EXTRA_COMPOSE_ARGS[@]}" )
  fi
}

show_summary() {
  local stack_dir config_file
  stack_dir="$STACK_ROOT/runtime"
  config_file="$CONFIG_ROOT/config.toml"
  cat <<SUMMARY

${LOG_PREFIX} Installation complete.
  Image tag:     $IMAGE_NAME
  Stack root:    $stack_dir
  Config file:   $config_file
  Systemd unit:  $( [[ $SYSTEMD_SETUP -eq 1 ]] && echo "$SYSTEMD_UNIT (enabled)" || echo "not installed" )
  Install log:   $INSTALL_LOG

Next steps:
  - Edit the configuration at $config_file or via the setup wizard (http://<host>:8080).
  - View logs with: ${SUDO:-} journalctl -u $SYSTEMD_UNIT -f (systemd mode) or docker logs -f r-emsd.
  - Update the runtime by rerunning this script after pulling repository changes.
SUMMARY
}

main() {
  parse_args "$@"
  require_elevated_access
  setup_logging
  detect_platform
  ensure_prereq_tools

  if [[ $INSTALL_DOCKER -eq 1 ]]; then
    install_docker_engine
  else
    log "Skipping Docker installation (--skip-docker)"
  fi

  add_user_to_docker_group
  install_rust_toolchain
  verify_docker_access
  ensure_compose_plugin
  ensure_buildx_builder
  determine_app_version
  build_container_image
  stage_runtime_stack
  stage_native_units
  install_systemd_unit
  start_runtime
  show_summary
}

main "$@"
