FROM rust:1.82

COPY ./Cargo.toml ./
RUN echo "fn main() {}" > dummy.rs
RUN sed -i 's#src/main.rs#dummy.rs#' ./Cargo.toml
RUN cargo build --release
RUN sed -i 's#dummy.rs#src/main.rs#' ./Cargo.toml

COPY ./src ./src

RUN cargo build --release

COPY ./data ./data
COPY ./.creds ./.creds

ENV RUST_LOG=info
CMD ["./target/release/untitled"]
