# Jupiter CPI Swap Example

This repository contains practical program examples and implementations designed to help developers work with the Cross Program Invocation (CPI) for Jupiter programs.

Written in Rust.

## Build

Build the `cpi-swap-client` folder.

```bash
cd cpi-swap-client
cargo build
```

## Run

You can either:

1. Run `cpi-swap-client` with your `BS58_KEYPAIR`.
2. Run `cpi-swap-client` with your `KEYPAIR`.

- With your `BS58_KEYPAIR`

```bash
BS58_KEYPAIR=your_bs58_keypair cargo run
```

- With your `KEYPAIR`

```bash
KEYPAIR=your_keypair cargo run
```

## Run with RPC URL

```bash
RPC_URL=https://api.apr.dev BS58_KEYPAIR=your_bs58_keypair cargo run
```
