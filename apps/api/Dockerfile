# Shared Rust build stage: produces both api and indexer binaries from a single
# `cargo build` so the second image (indexer) is essentially a copy step.
#
# This Dockerfile defines two final targets:
#   - api      → /usr/local/bin/api + sqlx-cli
#   - indexer  → /usr/local/bin/indexer
#
# The CI builds it twice (one per --target). The shared `builder` stage hits
# the layer cache on the second build, so only the small final stage runs.

FROM rust:1-bookworm AS builder
WORKDIR /app

# Copy workspace manifests and create dummy source files to cache dependency builds
COPY Cargo.toml ./
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/indexer/Cargo.toml apps/indexer/Cargo.toml
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/notifications/Cargo.toml crates/notifications/Cargo.toml
COPY crates/parsers/Cargo.toml crates/parsers/Cargo.toml

RUN mkdir -p apps/api/src apps/indexer/src crates/core/src crates/notifications/src crates/parsers/src && \
    echo "fn main() {}" > apps/api/src/main.rs && \
    echo "fn main() {}" > apps/indexer/src/main.rs && \
    echo "" > apps/indexer/src/lib.rs && \
    echo "" > crates/core/src/lib.rs && \
    echo "" > crates/notifications/src/lib.rs && \
    echo "" > crates/parsers/src/lib.rs

# Build dependencies only — both binaries at once so workspace deps are compiled
# only one time (cached as long as Cargo.toml files don't change).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p api -p indexer && \
    cargo install sqlx-cli --no-default-features --features postgres --locked

# Copy real source code and build both binaries in one pass
COPY apps/api/src apps/api/src
COPY apps/indexer/src apps/indexer/src
COPY crates/core/src crates/core/src
COPY crates/notifications/src crates/notifications/src
COPY crates/parsers/src crates/parsers/src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    find apps/ crates/ -name '*.rs' -exec touch {} + && \
    cargo build --release -p api -p indexer && \
    cp /app/target/release/api /usr/local/bin/api && \
    cp /app/target/release/indexer /usr/local/bin/indexer

# Common runtime base used by both api and indexer (pdfium + ca-certs)
FROM debian:bookworm-slim AS runtime-base

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

# Download pdfium shared library (replaces pdftoppm subprocess)
RUN ARCH=$(dpkg --print-architecture) && \
    case "$ARCH" in \
        amd64) PDFIUM_ARCH="linux-x64" ;; \
        arm64) PDFIUM_ARCH="linux-arm64" ;; \
        *) echo "Unsupported arch: $ARCH" && exit 1 ;; \
    esac && \
    wget -q "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-${PDFIUM_ARCH}.tgz" -O /tmp/pdfium.tgz && \
    tar -xzf /tmp/pdfium.tgz -C /tmp && \
    cp /tmp/lib/libpdfium.so /usr/local/lib/ && \
    rm -rf /tmp/pdfium.tgz /tmp/lib /tmp/include && \
    ldconfig

# ─── api target ────────────────────────────────────────────────────────────
FROM runtime-base AS api

RUN apt-get update && apt-get install -y --no-install-recommends \
    locales postgresql-client \
    && rm -rf /var/lib/apt/lists/* \
    && sed -i '/en_US.UTF-8/s/^# //g' /etc/locale.gen && locale-gen
ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8

COPY --from=builder /usr/local/bin/api /usr/local/bin/api
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY infra/migrations /app/migrations
COPY apps/api/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh
EXPOSE 7080
CMD ["/usr/local/bin/entrypoint.sh"]

# ─── indexer target ────────────────────────────────────────────────────────
FROM runtime-base AS indexer

COPY --from=builder /usr/local/bin/indexer /usr/local/bin/indexer
EXPOSE 7081
CMD ["/usr/local/bin/indexer"]
