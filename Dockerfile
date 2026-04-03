# Stage 1: Build Rust backend
FROM rust:1.85-slim AS backend-builder
WORKDIR /app/backend

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    pkg-config \
    libgit2-dev \
    cmake \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml backend/Cargo.lock* ./
COPY backend/src ./src
RUN cargo build --release

# Stage 2: Build SvelteKit frontend with Bun
FROM oven/bun:1.3.11 AS frontend-builder
WORKDIR /app/frontend

COPY frontend/package.json frontend/bun.lock ./
RUN bun install --frozen-lockfile

COPY frontend/ .
RUN bun run build

# Stage 3: Runtime image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libgit2-1.5 \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=backend-builder /app/backend/target/release/githree /usr/local/bin/githree
COPY --from=frontend-builder /app/frontend/build /app/static
COPY config/ /app/config/

WORKDIR /app
VOLUME ["/app/data"]
EXPOSE 3001
ENV RUST_LOG=info

CMD ["githree"]
