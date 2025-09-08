### Builder stage: compile cigen from source
FROM rust:1.88 AS builder
WORKDIR /app

# Copy source and build
COPY . .
RUN cargo build --release --locked --bin cigen

### Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends --no-upgrade \
    ca-certificates curl jq bash git \
 && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*

# Copy compiled binary from builder
COPY --from=builder /app/target/release/cigen /usr/local/bin/cigen

ENTRYPOINT ["cigen"]
