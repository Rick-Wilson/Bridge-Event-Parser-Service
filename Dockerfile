# syntax=docker/dockerfile:1.7

FROM rust:1-bookworm AS builder
WORKDIR /workspace

COPY Cargo.toml Cargo.lock* ./bridge-event-parser-service/
WORKDIR /workspace/bridge-event-parser-service
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/deps/bridge_event_parser_service*

COPY src/ ./src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget mdbtools \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1000 -m service

USER service
WORKDIR /app
COPY --from=builder /workspace/bridge-event-parser-service/target/release/bridge-event-parser-service /app/bridge-event-parser-service

ENV PORT=3001
EXPOSE 3001

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD wget -q --spider http://localhost:3001/healthz || exit 1

CMD ["/app/bridge-event-parser-service"]
