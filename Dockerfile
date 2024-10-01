FROM rust:1.81

COPY ./Cargo.toml ./
COPY ./src ./src

RUN cargo build --release
ENV RUST_LOG=info
CMD ["./target/release/untitled"]
