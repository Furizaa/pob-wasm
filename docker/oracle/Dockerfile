# Runs oracle parity tests: compares Rust engine output against PoB ground truth.
FROM rust:1.77-slim-bookworm

RUN apt-get update && apt-get install -y \
    luajit \
    git \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build --workspace --exclude pob-wasm --release

ENV DATA_DIR=/app/data

CMD ["cargo", "test", "--package", "pob-calc", "--test", "oracle", "--release", "--", "--ignored", "--nocapture"]
