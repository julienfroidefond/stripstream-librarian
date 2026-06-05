# Two-stage Rust build using cargo-chef for reliable layer caching.
#
# cargo-chef separates dependency compilation (slow, rarely changes) from
# application compilation (fast, changes every commit). The cooked deps layer
# is exported to the registry cache so builds are fast even after a cold start.
#
# Targets:
#   - api      → /usr/local/bin/api + sqlx-cli
#   - indexer  → /usr/local/bin/indexer

FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm@sha256:a26ffd9b429500b0c3f818fe18e876212eb6264689a7b973dcc6a40a11fb8153 AS chef
WORKDIR /app

# ─── Planner: compute dependency recipe ──────────────────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ─── sqlx-cli: separate stage so app rebuilds never retrigger this ───────────
FROM chef AS sqlx-installer
RUN cargo install sqlx-cli --no-default-features --features postgres --locked

# ─── Builder ─────────────────────────────────────────────────────────────────
FROM chef AS builder

# mold: much faster linker (~30s → ~3s link step)
RUN apt-get update && apt-get install -y --no-install-recommends mold \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold"

# Cook deps — this layer is cached by recipe.json (= Cargo.toml + Cargo.lock).
# Exported to the registry cache so a cold-start CI still hits this layer.
# crates/ must be present before cooking: cargo-chef needs the Cargo.toml of
# path dependencies (e.g. core2-shim) to resolve the workspace graph.
COPY --from=planner /app/recipe.json recipe.json
COPY crates/ ./crates/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json

# Build application (only workspace crates recompile on source changes)
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release -p api -p indexer && \
    cp target/release/api /usr/local/bin/api && \
    cp target/release/indexer /usr/local/bin/indexer

# ─── Common runtime base ─────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime-base

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

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

# ─── api target ──────────────────────────────────────────────────────────────
FROM runtime-base AS api

RUN apt-get update && apt-get install -y --no-install-recommends \
    locales postgresql-client \
    && rm -rf /var/lib/apt/lists/* \
    && sed -i '/en_US.UTF-8/s/^# //g' /etc/locale.gen && locale-gen
ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8

COPY --from=builder /usr/local/bin/api /usr/local/bin/api
COPY --from=sqlx-installer /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY infra/migrations /app/migrations
COPY apps/api/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh
EXPOSE 7080
CMD ["/usr/local/bin/entrypoint.sh"]

# ─── indexer target ──────────────────────────────────────────────────────────
FROM runtime-base AS indexer

COPY --from=builder /usr/local/bin/indexer /usr/local/bin/indexer
EXPOSE 7081
CMD ["/usr/local/bin/indexer"]
