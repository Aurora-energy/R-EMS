FROM rust:1.83-slim AS builder

# Install build dependencies required by transitive crates. In particular the
# supervisor depends on `reqwest`, which pulls in `openssl-sys`. Without the
# pkg-config utility and OpenSSL development headers the build fails with an
# "openssl-sys: Could not find directory of OpenSSL installation" error.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY services/supervisor/Cargo.toml services/supervisor/Cargo.toml
COPY services/bus/Cargo.toml services/bus/Cargo.toml
COPY services/registry/Cargo.toml services/registry/Cargo.toml
COPY services/configd/Cargo.toml services/configd/Cargo.toml
COPY services/logger/Cargo.toml services/logger/Cargo.toml
COPY services/gui/Cargo.toml services/gui/Cargo.toml
COPY services/schemas/Cargo.toml services/schemas/Cargo.toml
# Seed placeholder sources for each workspace crate so dependency resolution can
# run before the repository sources are copied into the build context.
COPY scripts/docker/prepare-workspace.sh /tmp/prepare-workspace.sh
RUN chmod +x /tmp/prepare-workspace.sh && /tmp/prepare-workspace.sh
RUN cargo fetch
COPY services services
RUN cargo build -p r-ems-supervisor --release
FROM gcr.io/distroless/cc
COPY --from=builder /build/target/release/r-ems-supervisor /usr/local/bin/r-ems-supervisor
ENTRYPOINT ["/usr/local/bin/r-ems-supervisor"]
