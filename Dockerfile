FROM rust:1-bookworm as builder

ARG DATABASE_URL

WORKDIR /app

RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release

FROM rust:1-bookworm as runner

WORKDIR /app

RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rusty-api /app/rusty-api

CMD [ "/app/rusty-api" ]
