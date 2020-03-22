# This Dockerfile is intended to automate cross compiling 
# vault-inject for macOS. Derived from:
# https://wapl.es/rust/2019/02/17/rust-cross-compile-linux-to-macos.html)
FROM rust:1.42.0

# Install deps for cross compiling to macos:
RUN apt update \
 && apt install -y \
        clang \
        cmake \
        gcc \
        g++ \
        zlib1g-dev \
        libmpc-dev \
        libmpfr-dev \
        libgmp-dev

# Add mac target:
RUN rustup target add x86_64-apple-darwin

# Install linker and such that we'll be using (.cargo/config points at these):
RUN git clone https://github.com/tpoechtrager/osxcross \
 && cd osxcross \
 && wget -nc https://s3.dockerproject.org/darwin/v2/MacOSX10.10.sdk.tar.xz \
 && mv MacOSX10.10.sdk.tar.xz tarballs/ \
 && UNATTENDED=yes OSX_VERSION_MIN=10.7 ./build.sh

# Point to the relevant compilers and such for any C code we need to compile:
ENV PATH=/osxcross/target/bin:$PATH
ENV CC=o64-clang
ENV CXX=o64-clang++

# Assume the dockerfile is run from its own folder. Copy across the cargo
# config we'll need for the MacOS build:
RUN mkdir /.cargo
COPY cargo_config /.cargo/config

# Assume code is mounted at /code and build a macos target:
RUN mkdir /code
WORKDIR /code
CMD ["cargo", "build", "--target", "x86_64-apple-darwin", "--release"]
