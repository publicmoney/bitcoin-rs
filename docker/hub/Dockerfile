FROM ubuntu:xenial

# install tools and dependencies
RUN apt update && apt install -y --no-install-recommends openssl

# show backtraces
ENV RUST_BACKTRACE 1

# cleanup Docker image
RUN apt autoremove -y \
  && apt clean -y \
  && rm -rf /tmp/* /var/tmp/* /var/lib/apt/lists/*

RUN groupadd -g 1000 bitcoin-rs-ubuntu \
  && useradd -m -u 1000 -g bitcoin-rs-ubuntu -s /bin/sh bitcoin-rs-ubuntu

WORKDIR /home/bitcoin-rs-ubuntu

COPY artifacts/$CARGO_TARGET/bitcoin-rs /bin/bitcoin-rs-ubuntu

COPY scripts/docker/hub/check_sync.sh /check_sync.sh

# switch to user bitcoin-rs-ubuntu here
USER bitcoin-rs-ubuntu

# setup ENTRYPOINT
EXPOSE 8333 18333 8332 18332
ENTRYPOINT ["/bin/bitcoin-rs-ubuntu"]
