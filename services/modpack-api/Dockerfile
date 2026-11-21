# syntax=docker/dockerfile:1
FROM rust:1.95.0-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY services ./services
COPY tools/xtask ./tools/xtask
COPY apps/desktop/src-tauri ./apps/desktop/src-tauri
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    find crates services tools/xtask apps/desktop/src-tauri -type f -exec touch {} + && \
    cargo build --locked --release -p slate-modpack-api && \
    cp target/release/slate-modpack-api /slate-modpack-api

FROM debian:bookworm-slim AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    useradd --system --uid 10001 --create-home slate
COPY --from=builder /slate-modpack-api /usr/local/bin/slate-modpack-api
USER slate
ENV SLATE_API_BIND=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/slate-modpack-api"]
