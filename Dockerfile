FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock ./
COPY src-tauri/src ./src

# Build only the headless binary (no GUI deps needed)
RUN cargo build --release --bin cc-proxy

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cc-proxy /usr/local/bin/cc-proxy
COPY config.toml /etc/cc-proxy/config.toml

EXPOSE 9528

ENV CC_PROXY_HEADLESS=1
ENTRYPOINT ["cc-proxy", "--headless", "--config", "/etc/cc-proxy/config.toml", "--host", "0.0.0.0"]
