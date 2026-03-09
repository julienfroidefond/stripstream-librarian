FROM rust:1-bookworm AS builder
WORKDIR /app

# Install sccache for faster builds
RUN cargo install sccache --locked
ENV RUSTC_WRAPPER=sccache
ENV SCCACHE_DIR=/sccache

COPY Cargo.toml ./
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/indexer/Cargo.toml apps/indexer/Cargo.toml
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/parsers/Cargo.toml crates/parsers/Cargo.toml
COPY apps/api/src apps/api/src
COPY apps/indexer/src apps/indexer/src
COPY crates/core/src crates/core/src
COPY crates/parsers/src crates/parsers/src

# Build with sccache (cache persisted between builds via Docker cache mount)
RUN --mount=type=cache,target=/sccache \
    cargo build --release -p indexer

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget unrar-free && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/indexer /usr/local/bin/indexer
EXPOSE 7081
CMD ["/usr/local/bin/indexer"]
