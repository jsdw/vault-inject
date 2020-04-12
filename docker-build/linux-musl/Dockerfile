FROM rust:1.42.0

RUN apt update && apt install -y musl-tools

RUN rustup target add x86_64-unknown-linux-musl

# Assume code is mounted at /code and build a macos target:
RUN mkdir /code
WORKDIR /code
CMD ["cargo", "build", "--target", "x86_64-unknown-linux-musl", "--release"]