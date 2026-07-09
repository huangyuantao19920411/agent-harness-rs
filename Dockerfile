# Build agent-harness-rs binary (coding-agent as default entrypoint)
FROM rust:1.85-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY examples ./examples
RUN cargo build --release -p coding-agent -p app-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/coding-agent /usr/local/bin/agent-harness
COPY --from=builder /app/target/release/app-server /usr/local/bin/harness-app-server
ENTRYPOINT ["agent-harness"]
