# The Bitcoin protocol implemented in Rust.

![CI](https://github.com/publicmoney/bitcoin-rs/workflows/CI/badge.svg)
[![license](https://img.shields.io/github/license/publicmoney/bitcoin-rs)](https://github.com/publicmoeny/bitcoin-rs/LICENSE)

This is a work in progress and is not recommended for production use.

All contributions are very welcome.

The goal is to reach some kind of feature parity with bitcoin-core and keep up with latest developments.

This was forked from https://github.com/paritytech/parity-bitcoin at the end of 2019 as the project was no longer active.

## Building from source

#### Install Rust

Install Rust and Cargo (build tool) from [rust-lang.org](https://www.rust-lang.org/tools/install).

#### Building bitcoin-rs

```
git clone https://github.com/publicmoney/bitcoin-rs
cd bitcoin-rs
cargo build --release
```
`bitcoin-rs` is now available at either `./target/debug/bitcoin-rs` or `./target/release/bitcoin-rs`.

Build for Arm64 (e.g Raspberry Pi) or many other targets using [Cross](https://github.com/rust-embedded/cross)

```
cargo install cross
cross build --target=aarch64-unknown-linux-gnu --release
```


## Running tests.

```
cargo test --all
```

## Going online

For a full list of CLI options run `bitcoin-rs --help`  

By default bitcoin-rs connects to bitcoin-core seednodes. Full list is available [here](bitcoin-rs/src/seednodes.rs).

To start syncing the main network, just start the client. For example:

```
./target/release/bitcoin-rs
```

## Importing bitcoind database

It is possible to import existing `bitcoind` database:

```
# where $BITCOIND_DB is path to your bitcoind database, e.g., "/Users/user/Library/Application Support"
./target/release/bitcoin-rs import "$BITCOIND_DB/Bitcoin/blocks"
```

By default import verifies imported the blocks. You can disable this, by adding `--verification-level==none` flag.

```
./target/release/bitcoin-rs import "#BITCOIND_DB/Bitcoin/blocks" --btc --skip-verification
```

## Logging

You can modify logging level on a per module basis by setting the environment variable `RUST_LOG`, e.g.,
```
RUST_LOG=sync=info,p2p=debug,verification=warn,db=trace ./target/release/bitcoin-rs
```
