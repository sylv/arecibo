FROM clux/muslrust:stable AS chef
USER root
RUN cargo install cargo-chef
WORKDIR /app



FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json



FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl --bin arecibo
RUN strip target/x86_64-unknown-linux-musl/release/arecibo



FROM alpine AS runtime
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/arecibo /usr/local/bin/
RUN addgroup -S arecibo && adduser -S arecibo -G arecibo
RUN mkdir -p /data && chown -R arecibo:arecibo /data
USER arecibo
ENV ARECIBO_DATA_DIR=/data
CMD ["/usr/local/bin/arecibo"]