# ==============================================================================
# STAGE 1: Prepare the dependency recipes (Fast Caching)
# ==============================================================================
FROM rust:slim AS planner
WORKDIR /app
RUN cargo install cargo-chef --version 0.1.66
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ==============================================================================
# STAGE 2: Build and cache external dependencies with full compiler tools
# ==============================================================================
FROM rust:slim AS builder
WORKDIR /app
# Install necessary tools for underlying C/C++ industrial bindings
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake g++ && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --version 0.1.66
COPY --from=planner /app/recipe.json recipe.json
# Pre-compile the exact workspace crate ecosystem dependency cache
RUN cargo chef cook --release --recipe-path recipe.json

# Copy the actual project source tree logic layers over
COPY . .
# Perform the final system release build compilation sweep
RUN cargo build --release --bin issem_core

# ==============================================================================
# STAGE 3: Hardened, Distroless Commercial Execution Runtime
# ==============================================================================
FROM gcr.io/distroless/cc-debian12:latest AS runtime
WORKDIR /app

# Copy the optimized, completely stripped binary from the builder layer
COPY --from=builder /app/target/release/issem_core /app/issem_core

# Establish production environment log variables and expose target Zenoh listener portals
ENV RUST_LOG=info
EXPOSE 7447

# Execute the binary directly (no shell wrapper available)
ENTRYPOINT ["/app/issem_core"]