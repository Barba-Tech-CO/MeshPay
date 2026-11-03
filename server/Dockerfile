FROM rust:1.99-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migrations migrations
COPY src src
ARG CARGO_FEATURES
RUN cargo build --release --locked --features "$CARGO_FEATURES"

FROM debian:12.15-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends curl ca-certificates \
  && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/meshpay_server /usr/local/bin/meshpay_server
USER nobody
CMD ["meshpay_server"]
