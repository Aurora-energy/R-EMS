# ---
# ems_section: "10-deployment-ci-cd-enhancements"
# ems_subsection: "docker-install-windows"
# ems_type: "script"
# ems_scope: "operations"
# ems_description: "Helper to install Docker prerequisites on Windows hosts."
# ems_version: "v0.0.0-prealpha"
# ems_owner: "tbd"
# ---
param(
    [string]$Image = "r-emsd:latest",
    [string]$Container = "r-emsd",
    [string[]]$Args
)

$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$configDir = Join-Path $projectRoot "target/docker-config"
$logDir = Join-Path $projectRoot "target/docker-logs"
$snapshotDir = Join-Path $projectRoot "target/docker-snapshots"

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Write-Error "Docker is not installed or not available on PATH."
    exit 1
}

Write-Host "[docker-install] Building image $Image"
docker build -t $Image $projectRoot

New-Item -ItemType Directory -Force -Path $configDir, $logDir, $snapshotDir | Out-Null
$configPath = Join-Path $configDir "config.toml"
if (-not (Test-Path $configPath)) {
    Copy-Item (Join-Path $projectRoot "configs/docker.default.toml") $configPath
}

$existing = docker ps -a --format '{{.Names}}' | Where-Object { $_ -eq $Container }
if ($existing) {
    Write-Host "[docker-install] Removing existing container $Container"
    docker rm -f $Container | Out-Null
}

$resolvedConfig = (Resolve-Path $configDir).Path
$resolvedLogs = (Resolve-Path $logDir).Path
$resolvedSnapshots = (Resolve-Path $snapshotDir).Path

Write-Host "[docker-install] Starting container $Container"
docker run -d `
    --name $Container `
    --restart unless-stopped `
    -e R_EMS_CONFIG_PATH=/data/config/config.toml `
    -e R_EMS_LICENSE_BYPASS=1 `
    -e R_EMS_LOG=debug `
    -v "$resolvedConfig:/data/config" `
    -v "$resolvedLogs:/data/logs" `
    -v "$resolvedSnapshots:/data/snapshots" `
    $Image @Args | Out-Null

Write-Host "[docker-install] Container $Container is running. View logs with: docker logs -f $Container"