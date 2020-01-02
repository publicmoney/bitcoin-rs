# This Dockerfile uses Docker Multi-Stage Builds
# See https://docs.docker.com/engine/userguide/eng-image/multistage-build/

### Base Image
# Setup up a base image to use in Build and Runtime images
FROM rust:1.34-stretch AS build

# rustup directory
ENV PATH=/root/.cargo/bin:$PATH \
    RUST_BACKTRACE=1

WORKDIR /build/

# install tools and dependencies
RUN apt-get update && \
        apt-get install -y --force-yes --no-install-recommends \
        g++ \
        build-essential \
        git \
        ca-certificates \
        libssl-dev \
        pkg-config \
        libudev-dev

# show tools
RUN rustc -vV
RUN cargo -V
RUN gcc -v
RUN g++ -v

# build bitcoin-rs
RUN git clone https://github.com/publicmoney/bitcoin-rs.git; \
    cd publicmoney; export RUSTFLAGS=" -C link-arg=-s"; \
    cargo build --release --verbose 

# Runtime image, copies bitcoin-rs artifact from build image
FROM ubuntu:bionic AS run

WORKDIR /bitcoin-rs-ubuntu
COPY --from=build /build/bitcoin-rs/target/release/bitcoin-rs /bitcoin-rs-ubuntu/

EXPOSE 8333 18333 8332 18332
ENTRYPOINT ["/bitcoin-rs-ubuntu/bitcoin-rs"]
