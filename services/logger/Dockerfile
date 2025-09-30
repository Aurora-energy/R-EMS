FROM rust:1.83-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
# Provide the manifests for every workspace member so dependency resolution can
# run without copying the entire repository yet.
COPY services/bus/Cargo.toml services/bus/Cargo.toml
COPY services/configd/Cargo.toml services/configd/Cargo.toml
COPY services/gui/Cargo.toml services/gui/Cargo.toml
COPY services/logger/Cargo.toml services/logger/Cargo.toml
COPY services/registry/Cargo.toml services/registry/Cargo.toml
COPY services/schemas/Cargo.toml services/schemas/Cargo.toml
COPY services/supervisor/Cargo.toml services/supervisor/Cargo.toml

COPY scripts/docker/prepare-workspace.sh /tmp/prepare-workspace.sh
RUN chmod +x /tmp/prepare-workspace.sh && /tmp/prepare-workspace.sh

RUN cargo fetch
COPY services services
RUN cargo build -p r-ems-logger --release

FROM gcr.io/distroless/cc
COPY --from=builder /build/target/release/r-ems-logger /usr/local/bin/r-ems-logger
ENTRYPOINT ["/usr/local/bin/r-ems-logger"]
