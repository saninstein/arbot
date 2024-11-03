FROM rust:1.82

COPY ./Cargo.toml ./
COPY ./src ./src

RUN cargo build --release

COPY ./data ./data
COPY ./.creds ./.creds

ENV RUST_LOG=info
CMD ["./target/release/untitled"]
