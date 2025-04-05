# Jupiter CPI Swap Example

This repository contains practical program examples and implementations designed to help developers work with the Cross Program Invocation (CPI) for Jupiter programs.

Written in Rust.

## Setup

CD into the `cpi-swap-client` directory and copy the `.env.example` file to `.env` and add your private key or keypair.

```bash
cd cpi-swap-client
cp .env.example .env
```

## Build

Build the `cpi-swap-client` folder.

```bash
cd cpi-swap-client
cargo build
```

## Run

You can either:

1. Run the `cpi-swap-client` folder with the `.env` variables.

```bash
source .env
cargo run
```

2. Run the `cpi-swap-client` folder with your `BS58_KEYPAIR` or `KEYPAIR` directly in terminal.

- With your `BS58_KEYPAIR`

```bash
BS58_KEYPAIR=your_bs58_keypair cargo run
```

- With your `KEYPAIR`

```bash
KEYPAIR=your_keypair cargo run
```
