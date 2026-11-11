# Leak news server + website in one small image.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY server/Cargo.toml server/Cargo.lock ./
# Cache dependencies separately from source.
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release --locked && rm -rf src
COPY server/src ./src
RUN touch src/main.rs && cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /src/target/release/leak /usr/local/bin/leak
COPY server/sources.toml /app/sources.toml
COPY web /app/web
ENV LEAK_SOURCES=/app/sources.toml \
    LEAK_WEB_DIR=/app/web \
    LEAK_DATA_DIR=/data \
    PORT=8080 \
    RUST_LOG=leak=info
VOLUME /data
EXPOSE 8080
CMD ["leak"]
