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

COPY --from=builder /app/target/release/indexer /usr/local/bin/indexer
EXPOSE 7081
CMD ["/usr/local/bin/indexer"]
