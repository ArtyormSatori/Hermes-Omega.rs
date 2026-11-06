FROM rust:1.90-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev build-essential sqlite3 libsqlite3-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/hermes-omega-rs
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git sqlite3 bash tini \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 -s /bin/bash hermes

WORKDIR /home/hermes
COPY --from=builder /usr/src/hermes-omega-rs/target/release/hermes-omega /usr/local/bin/hermes-omega
COPY config.yaml /home/hermes/.hermes/config.yaml
COPY SOUL.md /home/hermes/.hermes/SOUL.md
COPY MEMORY.md /home/hermes/.hermes/MEMORY.md
COPY USER.md /home/hermes/.hermes/USER.md

RUN mkdir -p /home/hermes/.hermes/skills \
             /home/hermes/.hermes/memory_tree/wiki \
             /home/hermes/.hermes/trajectories \
             /home/hermes/workspace \
    && chown -R hermes:hermes /home/hermes

USER hermes
ENV HERMES_HOME="/home/hermes/.hermes"
ENV RUST_LOG="info,hermes_omega_rs=debug"

EXPOSE 8080
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/hermes-omega"]
CMD ["daemon", "--bind", "0.0.0.0:8080"]
