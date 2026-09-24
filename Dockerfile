FROM rust:1.98.1-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
# Query metadata is generated against a disposable database and checked in.
# Building an image must never connect to, or migrate, a production database.
ENV SQLX_OFFLINE=true
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runner
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home envx
WORKDIR /app
COPY --from=builder /app/target/release/rusty-api /app/rusty-api
USER envx
CMD ["/app/rusty-api"]
