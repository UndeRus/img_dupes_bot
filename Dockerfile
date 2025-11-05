FROM clux/muslrust:1.91.0-stable AS chef
USER root
RUN cargo install cargo-chef
WORKDIR /app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS rust-builder
COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json

COPY . /volume
RUN cd /volume && cargo build --release

FROM gcr.io/distroless/cc-debian12

COPY --from=rust-builder /volume/target/x86_64-unknown-linux-musl/release/img_bot /bot
CMD ["./bot"]