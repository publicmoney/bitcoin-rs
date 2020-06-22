# An implementation of the ₿itcoin protocol written in Rust.

![CI](https://github.com/publicmoney/bitcoin-rs/workflows/CI/badge.svg)
[![license](https://img.shields.io/github/license/publicmoney/bitcoin-rs)](https://github.com/publicmoeny/bitcoin-rs/LICENSE)

This is a work in progress and is not recommended for production use.

All contributions are very welcome.

The goal is to reach some kind of feature parity with bitcoin-core and keep up with latest developments.

This was forked from https://github.com/paritytech/parity-bitcoin at the end of 2019 as the project was no longer active.

- [Building from source](#building-from-source)

- [Running tests](#running-tests)

- [Going online](#going-online)

- [Importing bitcoind database](#importing-bitcoind-database)

- [Command line interface](#command-line-interface)

- [JSON-RPC](rpc/README.md)

- [Logging](#logging)

- [Internal Documentation](#internal-documentation)


## Building from source

#### Install Rust

Building `bitcoin-rs` from source requires `rustc` and `cargo`.

Install from [rust-lang.org](https://www.rust-lang.org/tools/install).

#### Install C and C++ compilers

You will need the cc and gcc compilers to build some of the dependencies.

```
sudo apt-get update
sudo apt-get install build-essential
```

#### Clone and build bitcoin-rs

Now let's clone `bitcoin-rs` and enter it's directory:

```
git clone https://github.com/publicmoney/bitcoin-rs
cd bitcoin-rs
```

`bitcoin-rs` can be build in two modes. `--debug` and `--release`. Debug is the default.

```
# builds bitcoin-rs in debug mode
cargo build -p bitcoin-rs
```

```
# builds bitcoin-rs in release mode
cargo build -p bitcoin-rs --release
```

`bitcoin-rs` is now available at either `./target/debug/bitcoin-rs` or `./target/release/bitcoin-rs`.

## Running tests

`bitcoin-rs` has internal unit tests and it conforms to external integration tests.

#### Running unit tests

Assuming that repository is already cloned, we can run unit tests with this command:

```
cargo test --all
```

#### Running external integration tests

Running integration tests is automated, as the regtests repository is one of the submodules. Let's download it first:

```
git submodule update --init
```

Now we can run them using the command:

```
./tools/regtests.sh
```

It is also possible to run regtests manually:

```
# let's start bitcoin-rs in regtest compatible mode
./target/release/bitcoin-rs --regtest

# now in second shell window
cd $HOME
git clone https://github.com/TheBlueMatt/test-scripts
cd test-scripts
java -jar pull-tests-f56eec3.jar

```

## Going online

By default bitcoin-rs connects to bitcoin-core seednodes. Full list is available [here](bitcoin-rs/seednodes.rs).

To start syncing the main network, just start the client, passing selected fork flag. For example:

```
./target/release/bitcoin-rs
```

To start syncing the testnet:

```
./target/release/bitcoin-rs --testnet
```

To not print any syncing progress add `--quiet` flag:

```
./target/release/bitcoin-rs --quiet
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

## Command line interface

Full list of CLI options, which is available under `bitcoin-rs --help`:

```
bitcoin-rs 0.1.0
Bitcoin client

USAGE:
    bitcoin-rs [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
        --btc             Use Bitcoin Core verification rules (BTC).
    -h, --help            Prints help information
        --no-jsonrpc      Disable the JSON-RPC API server.
    -q, --quiet           Do not show any synchronization information in the console.
        --regtest         Use a private network for regression tests.
        --testnet         Use the test network (Testnet3).
    -V, --version         Prints version information

OPTIONS:
        --blocknotify <COMMAND>            Execute COMMAND when the best block changes (%s in COMMAND is replaced by the block hash).
    -c, --connect <IP>                     Connect only to the specified node.
    -d, --data-dir <PATH>                  Specify the database and configuration directory PATH.
        --db-cache <SIZE>                  Sets the database cache size.
        --jsonrpc-apis <APIS>              Specify the APIs available through the JSONRPC interface. APIS is a comma-delimited list of API names.
        --jsonrpc-cors <URL>               Specify CORS header for JSON-RPC API responses.
        --jsonrpc-hosts <HOSTS>            List of allowed Host header values.
        --jsonrpc-interface <INTERFACE>    The hostname portion of the JSONRPC API server.
        --jsonrpc-port <PORT>              Specify the PORT for the JSONRPC API server.
        --only-net <NET>                   Only connect to nodes in network version <NET> (ipv4 or ipv6).
        --port <PORT>                      Listen for connections on PORT.
    -s, --seednode <IP>                    Connect to a seed-node to retrieve peer addresses, and disconnect.
        --verification-edge <BLOCK>        Non-default verification-level is applied until a block with given hash is met.
        --verification-level <LEVEL>       Sets the Blocks verification level to full (default), header (scripts are not verified), or none (no verification at all).

SUBCOMMANDS:
    help        Prints this message or the help of the given subcommand(s)
    import      Import blocks from a Bitcoin Core database.
    rollback    Rollback the database to given canonical-chain block.
```

## Logging

This is a section only for developers and power users.

You can enable detailed client logging by setting the environment variable `RUST_LOG`, e.g.,

```
RUST_LOG=verification=info ./target/release/bitcoin-rs --btc
```

`bitcoin-rs` started with this environment variable will print all logs coming from `verification` module with verbosity `info` or higher. Available log levels are:

- `error`
- `warn`
- `info`
- `debug`
- `trace`

It's also possible to start logging from multiple modules in the same time:

```
RUST_LOG=sync=trace,p2p=trace,verification=trace,db=trace ./target/release/bitcoin-rs
```

## Internal documentation
```
cd bitcoin-rs
./tools/doc.sh
open target/doc/bitcoin-rs/index.html
```
