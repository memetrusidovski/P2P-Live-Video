# Multi-stage build for the BitStream peer runtime (bs-node).
# Stage 1: build bsnode in release mode.
FROM rust:1-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release -p bs-node

# Stage 2: slim runtime with iproute2 so netem.sh can shape the container's link.
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends iproute2 ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/bsnode /usr/local/bin/bsnode
COPY fleet/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh
VOLUME ["/results"]
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
