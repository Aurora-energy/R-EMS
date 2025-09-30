# ---------------------------------------------------------------------------
# Stage 1: Builder
# ---------------------------------------------------------------------------
# We start from the official Rust image which contains the compiler toolchain
# and build dependencies. Using the `slim` variant keeps the image small while
# still providing necessary system libraries.
FROM rust:1.83-slim AS builder

# Install OS packages required to build native dependencies (if any). We keep
# the list minimal to maintain a small attack surface. `ca-certificates` is
# required for the `reqwest` client to talk to HTTPS endpoints during runtime.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a new empty shell project to leverage Cargo's layer caching. Copying
# the manifest files first ensures that dependency compilation is cached unless
# they change.
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY services/bus/Cargo.toml services/bus/Cargo.toml
COPY services/configd/Cargo.toml services/configd/Cargo.toml
COPY services/gui/Cargo.toml services/gui/Cargo.toml
COPY services/logger/Cargo.toml services/logger/Cargo.toml
COPY services/registry/Cargo.toml services/registry/Cargo.toml
COPY services/schemas/Cargo.toml services/schemas/Cargo.toml
COPY services/supervisor/Cargo.toml services/supervisor/Cargo.toml

# Generate placeholder sources for all workspace members so dependency fetching
# works without the full repository.
COPY scripts/docker/prepare-workspace.sh /tmp/prepare-workspace.sh
RUN chmod +x /tmp/prepare-workspace.sh && /tmp/prepare-workspace.sh

# Pre-fetch dependencies so subsequent builds only recompile the changed code.
RUN cargo fetch

# Now copy the full source tree and build the release binary.
COPY services services
RUN cargo build -p r-ems-gui --release

# ---------------------------------------------------------------------------
# Stage 2: Runtime image
# ---------------------------------------------------------------------------
# We use the distroless `cc` image to run the compiled binary. Distroless images
# contain only the dynamic linker and C runtime, minimizing the attack surface.
FROM gcr.io/distroless/cc

# Copy the compiled binary from the builder stage. The binary resides in the
# target directory, which is part of the workspace build output.
COPY --from=builder /build/target/release/r-ems-gui /usr/local/bin/r-ems-gui

# Set the documentation directory as a volume mount point. Operators can bind
# their own documentation into `/srv/r-ems/docs` when running the container.
VOLUME ["/srv/r-ems/docs"]

# Define the default command. Environment variables can override the bind
# address and health endpoints.
ENTRYPOINT ["/usr/local/bin/r-ems-gui"]
