# ---
# ems_section: "07-install"
# ems_subsection: "01-docker"
# ems_type: "config"
# ems_scope: "install"
# ems_description: "Multi-stage Dockerfile for packaging the R-EMS daemon and setup wizard assets."
# ems_version: "v0.1.0"
# ems_owner: "tbd"
# reference: docs/VERSIONING.md
# ---

ARG RUST_VERSION=1.82
FROM rust:${RUST_VERSION} AS builder
ARG APP_VERSION="dev"
ENV APP_VERSION=${APP_VERSION}
WORKDIR /app
COPY . .
RUN cargo build --release --bin r-emsd

FROM debian:stable-slim
ARG APP_VERSION="dev"
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -d /var/lib/r-ems -s /usr/sbin/nologin rems
WORKDIR /opt/r-ems

LABEL org.opencontainers.image.title="R-EMS daemon" \
      org.opencontainers.image.description="Redundant energy management system runtime and setup wizard" \
      org.opencontainers.image.version="${APP_VERSION}" \
      org.opencontainers.image.vendor="tbd" \
      org.opencontainers.image.source="https://github.com/kentthoresen/R-EMS"

COPY --from=builder /app/target/release/r-emsd /usr/local/bin/r-emsd
COPY configs /opt/r-ems/configs
COPY ui/setup-wizard/public /opt/r-ems/ui/setup-wizard/public
COPY docker/entrypoint.sh /opt/r-ems/entrypoint.sh
RUN chmod +x /opt/r-ems/entrypoint.sh \
    && mkdir -p /data/config /data/logs /data/snapshots \
    && chown -R rems:rems /opt/r-ems /data

ENV R_EMS_CONFIG_SOURCE=/opt/r-ems/configs/docker.default.toml \
    R_EMS_CONFIG_PATH=/data/config/config.toml \
    R_EMS_LICENSE_BYPASS=1 \
    R_EMS_LOG=info \
    R_EMS_METRICS_LISTEN=0.0.0.0:9898 \
    R_EMS_API_LISTEN=0.0.0.0:8080

VOLUME ["/data/config", "/data/logs", "/data/snapshots"]
EXPOSE 8080 9898
USER rems
ENTRYPOINT ["/opt/r-ems/entrypoint.sh"]
CMD ["run"]

HEALTHCHECK --interval=30s --timeout=5s --retries=5 CMD curl -fsS http://127.0.0.1:8080/api/status || exit 1
