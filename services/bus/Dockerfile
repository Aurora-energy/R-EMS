# Builder stage compiles the Rust binary with release optimisations.
FROM rust:1.83-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
# Copy every workspace manifest so Cargo metadata resolution succeeds even
# before the real sources are present in the image build context.
COPY services/bus/Cargo.toml services/bus/Cargo.toml
COPY services/configd/Cargo.toml services/configd/Cargo.toml
COPY services/gui/Cargo.toml services/gui/Cargo.toml
COPY services/logger/Cargo.toml services/logger/Cargo.toml
COPY services/registry/Cargo.toml services/registry/Cargo.toml
COPY services/schemas/Cargo.toml services/schemas/Cargo.toml
COPY services/supervisor/Cargo.toml services/supervisor/Cargo.toml

# Generate placeholder source files for every crate in the workspace. This
# allows `cargo fetch` to run without needing the full source tree yet, which in
# turn keeps dependency caching effective.
COPY scripts/docker/prepare-workspace.sh /tmp/prepare-workspace.sh
RUN chmod +x /tmp/prepare-workspace.sh && /tmp/prepare-workspace.sh

RUN cargo fetch
COPY services services
RUN cargo build -p r-ems-bus --release

# Runtime stage based on distroless for minimal attack surface.
FROM gcr.io/distroless/cc
COPY --from=builder /build/target/release/r-ems-bus /usr/local/bin/r-ems-bus
ENTRYPOINT ["/usr/local/bin/r-ems-bus"]
