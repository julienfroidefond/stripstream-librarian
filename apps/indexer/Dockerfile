FROM rust:1-bookworm AS builder
WORKDIR /app

# Copy workspace manifests and create dummy source files to cache dependency builds
COPY Cargo.toml ./
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/indexer/Cargo.toml apps/indexer/Cargo.toml
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/parsers/Cargo.toml crates/parsers/Cargo.toml

RUN mkdir -p apps/api/src apps/indexer/src crates/core/src crates/parsers/src && \
    echo "fn main() {}" > apps/api/src/main.rs && \
    echo "fn main() {}" > apps/indexer/src/main.rs && \
    echo "" > apps/indexer/src/lib.rs && \
    echo "" > crates/core/src/lib.rs && \
    echo "" > crates/parsers/src/lib.rs

# Build dependencies only (cached as long as Cargo.toml files don't change)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p indexer

# Copy real source code and build
COPY apps/api/src apps/api/src
COPY apps/indexer/src apps/indexer/src
COPY crates/core/src crates/core/src
COPY crates/parsers/src crates/parsers/src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p indexer && \
    cp /app/target/release/indexer /usr/local/bin/indexer

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

# Download pdfium shared library (replaces pdftoppm + pdfinfo subprocesses)
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

COPY --from=builder /usr/local/bin/indexer /usr/local/bin/indexer
EXPOSE 7081
CMD ["/usr/local/bin/indexer"]
