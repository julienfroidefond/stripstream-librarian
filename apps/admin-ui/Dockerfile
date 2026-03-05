FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml ./
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/indexer/Cargo.toml apps/indexer/Cargo.toml
COPY apps/admin-ui/Cargo.toml apps/admin-ui/Cargo.toml
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/parsers/Cargo.toml crates/parsers/Cargo.toml
COPY apps/api/src apps/api/src
COPY apps/indexer/src apps/indexer/src
COPY apps/admin-ui/src apps/admin-ui/src
COPY crates/core/src crates/core/src
COPY crates/parsers/src crates/parsers/src

RUN cargo build --release -p admin-ui

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/admin-ui /usr/local/bin/admin-ui
EXPOSE 8082
CMD ["/usr/local/bin/admin-ui"]
