# ---
# ems_section: "10-deployment-ci-cd-enhancements"
# ems_subsection: "docker-build"
# ems_type: "config"
# ems_scope: "infrastructure"
# ems_description: "Multi-stage Dockerfile for building the R-EMS daemon and UI bundles."
# ems_version: "v0.0.0-prealpha"
# ems_owner: "tbd"
# ---
# syntax=docker/dockerfile:1.6
ARG RUST_VERSION=1.82

FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-slim-bullseye AS builder
ARG APP_VERSION="dev"
ARG TARGETPLATFORM
WORKDIR /workspace

# Install build tooling and cache dependencies between builds.
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends pkg-config libssl-dev llvm clang ca-certificates curl git make; \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY bin ./bin
COPY configs ./configs
COPY ui ./ui
COPY docker ./docker
COPY scripts ./scripts
COPY tests ./tests
COPY examples ./examples
COPY ARCHITECTURE.md README.md .

# Determine the Rust target triple for the requested platform.
RUN set -eux; \
    case "${TARGETPLATFORM}" in \
      "linux/amd64")  CARGO_TARGET="x86_64-unknown-linux-gnu" ;; \
      "linux/arm64")  CARGO_TARGET="aarch64-unknown-linux-gnu" ;; \
      "linux/arm/v7") CARGO_TARGET="armv7-unknown-linux-gnueabihf" ;; \
      *) echo "Unsupported TARGETPLATFORM: ${TARGETPLATFORM}" >&2; exit 1 ;; \
    esac; \
    rustup target add "${CARGO_TARGET}"; \
    export CARGO_NET_OFFLINE=true; \
    cargo fetch; \
    unset CARGO_NET_OFFLINE; \
    cargo build --release --bin r-emsd --target "${CARGO_TARGET}"; \
    mkdir -p /out/bin /out/ui; \
    cp "target/${CARGO_TARGET}/release/r-emsd" /out/bin/r-emsd; \
    cp -R ui/setup-wizard/public /out/ui/setup-wizard; \
    printf '%s' "${CARGO_TARGET}" >/out/CARGO_TARGET

FROM --platform=$TARGETPLATFORM debian:stable-slim AS runtime
ARG APP_VERSION="dev"
ENV DEBIAN_FRONTEND=noninteractive

RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl tzdata; \
    rm -rf /var/lib/apt/lists/*

RUN useradd -r -d /var/lib/r-ems -s /usr/sbin/nologin rems
WORKDIR /opt/r-ems

LABEL org.opencontainers.image.title="R-EMS" \
      org.opencontainers.image.description="Redundant Energy Management System runtime with static setup UI." \
      org.opencontainers.image.version="${APP_VERSION}" \
      org.opencontainers.image.vendor="tbd" \
      org.opencontainers.image.source="https://github.com/kentthoresen/R-EMS"

COPY --from=builder /out/bin/r-emsd /usr/local/bin/r-emsd
COPY --from=builder /out/ui/setup-wizard /opt/r-ems/ui/setup-wizard
COPY configs /opt/r-ems/configs
COPY docker/entrypoint.sh /opt/r-ems/entrypoint.sh
RUN chmod +x /opt/r-ems/entrypoint.sh \
    && mkdir -p /data/config /data/logs /data/snapshots \
    && chown -R rems:rems /opt/r-ems /data

ENV R_EMS_CONFIG_SOURCE=/opt/r-ems/configs/docker.default.toml \
    R_EMS_CONFIG_PATH=/data/config/config.toml \
    R_EMS_LICENSE_BYPASS=1 \
    R_EMS_LOG=info \
    R_EMS_METRICS_LISTEN=0.0.0.0:9898 \
    R_EMS_API_LISTEN=0.0.0.0:8080 \
    R_EMS_VERSION=${APP_VERSION}

VOLUME ["/data/config", "/data/logs", "/data/snapshots"]
EXPOSE 8080 9898
USER rems
ENTRYPOINT ["/opt/r-ems/entrypoint.sh"]
CMD ["run"]

HEALTHCHECK --interval=30s --timeout=5s --retries=5 CMD curl -fsS http://127.0.0.1:8080/api/status || exit 1