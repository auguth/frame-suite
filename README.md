# FRAME Suite Node

[![License](https://img.shields.io/badge/license-MPL--2.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)](https://www.rust-lang.org/)
[![Substrate](https://img.shields.io/badge/Substrate-Framework-E6007A)](https://docs.polkadot.com/)
[![CI](https://github.com/auguth/frame-suite/actions/workflows/ci.yml/badge.svg)](https://github.com/auguth/frame-suite/actions/workflows/ci.yml)

A **Substrate-based blockchain node** built with the **FRAME Suite** ecosystem.

## Current Primitives

The current FRAME Suite provides **modular staking primitives**, composed of reusable roles, bonding, orchestration, and behavior models.

This node currently integrates the following FRAME Suite primitives:

* [`frame_suite`](./frame/suite): core traits, abstractions, and semantics
* [`frame_plugins`](./frame/plugins): pluggable behaviour models
* [`pallet_xp`](./pallets/xp): non-monetary quantifiable reputation system
* [`pallet_commitment`](./pallets/commitment): economic bonding primitives
* [`pallet_authors`](./pallets/authors): block authors role & stake management
* [`pallet_chain_manager`](./pallets/chain-manager): validator orchestration

> This set may evolve over time as new primitives are added to `frame_suite` and integrated into the node.

## Getting Started

### Build

Build the node:

```sh
cargo build --release
```

### Build with Dev Features

For development (enables additional logging, debugging, and optional features):

```sh
cargo build --release --features dev
```

### Run Development Chain

Start a local development chain:

```sh
./target/release/frame-suite-node --dev
```

### Run with Dev Features

```sh
cargo run --release --features dev -- --dev
```

### Purge Chain

```sh
./target/release/frame-suite-node purge-chain --dev
```

### Run with Logs

```sh
RUST_BACKTRACE=1 ./target/release/frame-suite-node --dev -l debug
```

## Structure

### Node

The [`node/`](./node) directory defines:

* networking (libp2p)
* consensus integration
* RPC interface

Key files:

* [service.rs](./node/src/service.rs): node service and consensus setup. 
* [rpc.rs](./node/src/rpc.rs): RPC API definitions. 
* [command.rs](./node/src/command.rs): CLI command execution. 
* [cli.rs](./node/src/cli.rs): CLI structure and subcommands. 
* [chain_spec.rs](./node/src/chain_spec.rs): chain configuration. 

### Runtime

The runtime defines blockchain logic:

* located in [`runtime/src/lib.rs`](./runtime/src/lib.rs)
* composed using FRAME pallets
* configured via `impl Config` blocks

Key files:

* [lib.rs](./runtime/src/lib.rs): Main runtime file and pallet setup. 
* [apis.rs](./runtime/src/apis.rs) Runtime API implementations. 
* [mod.rs](./runtime/src/configs/mod.rs): Runtime configuration for pallets. 
* [genesis_config_presets.rs](./runtime/src/genesis_config_presets.rs): Genesis accounts & pallet bootstrap-values. 

### Pallets

Custom pallets are located in [`pallets/`](./pallets):

* define storage, extrinsics, and logic
* are composed into the runtime


### FRAME

The [`frame/`](./frame) directory contains supporting crates used across the runtime:

* [`frame/suite`](./frame/suite): core traits and abstractions
* [`frame/plugins`](./frame/plugins): plugin models for configuring behavior

They act as the **foundation layer**, while pallets provide concrete runtime behavior.

## Development

### Modify Runtime

To:

* [mod.rs](./runtime/src/configs/mod.rs) to change plugin models and adjust parameters
* [lib.rs](./runtime/src/lib.rs) to add/remove pallets

### Plugins

Logical behaviors are configurable via [`frame_plugins`](./frame/plugins).
These are selected at the **runtime level**.

### Dev Feature

The `dev` feature enables:

* additional logging and event emissions
* debugging utilities via extrinsics
* optional development configurations

Use it during development:

```sh
cargo build --features dev
```

## Connect Frontend

Connect using Polkadot.js Apps:

```sh
ws://127.0.0.1:9944
```

## Documentation

Please refer to generated [Cargo-Docs](https://auguth.github.io/frame-suite/rust-doc/) for pallets and frame-utilities rust-documentation.

## Contributing

Please refer to [CONTRIBUTING.md](./CONTRIBUTING.md) for contribution guidelines.

## License

[MPL-2.0 (Mozilla Public License)](./LICENSE)

An Open-Source initiative by **Auguth Labs (OPC) Pvt Ltd, India**
