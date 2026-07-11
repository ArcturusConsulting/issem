# ==============================================================================
# STAGE 1: Prepare the dependency recipes
# ==============================================================================
FROM rust:slim AS planner
WORKDIR /app
RUN cargo install cargo-chef --version 0.1.66
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ==============================================================================
# STAGE 2: Build and cache external dependencies
# ==============================================================================
FROM rust:slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake g++ && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --version 0.1.66
COPY --from=planner /app/recipe.json recipe.json
# Build our external crate cache dependencies layer cleanly
RUN cargo chef cook --release --recipe-path recipe.json

# Copy the actual project logic layers over
COPY . .
# Perform the final system release build compilation sweep
RUN cargo build --release --bin issem_core

# ==============================================================================
# STAGE 3: Minimal, high-security execution runtime footprint
# ==============================================================================
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install standard SSL roots for secure network configurations
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Copy the optimized binary over from the builder target mirror
COPY --from=builder /app/target/release/issem_core /app/issem_core
COPY config.json /app/config.json

# Establish environment log variables and expose target Zenoh listener portals
ENV RUST_LOG=info
EXPOSE 7447

ENTRYPOINT ["/app/issem_core"]