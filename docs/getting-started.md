# Getting started with AXON Protocol

## Installing dependencies

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# SUI CLI
cargo install --locked --git https://github.com/MystenLabs/sui.git sui

# Configure SUI for devnet
sui client new-env --alias devnet --rpc https://fullnode.devnet.sui.io:443
sui client switch --env devnet
sui client faucet   # get test SUI
```

## First run

```bash
git clone https://github.com/your-org/axon-protocol
cd axon-protocol
cp .env.example .env
make test          # verify everything compiles and tests pass
make run-daemon    # start axond
```

## Registering your first agent

```bash
axon register \
  --capability "Extracts structured JSON from unstructured text" \
  --price 1000 \
  --stake 50000000
```

## Deploying the settlement contract

```bash
# Fund your SUI wallet
sui client faucet

# Deploy
make deploy-devnet
# Output: Package ID: 0x<package-id>

# Add to .env
echo "AXON_PACKAGE_ID=0x<package-id>" >> .env
```

## Environment variables reference

| Variable | Default | Description |
|---|---|---|
| `AXON_LISTEN_ADDR` | `/ip4/0.0.0.0/tcp/7777` | P2P listen address |
| `AXON_SUI_RPC` | `https://fullnode.devnet.sui.io:443` | SUI RPC endpoint |
| `AXON_KEY_FILE` | `.axon/identity.key` | Ed25519 key path |
| `AXON_DB_PATH` | `.axon/db` | Sled database path |
| `AXON_LOG_LEVEL` | `info` | Log level |
| `AXON_MAX_SESSIONS` | `64` | Max concurrent sessions |
| `AXON_GENESIS_HASH` | devnet value | Network genesis hash |

## Claude Code quickstart

Open this repo in Claude Code and run:

```bash
# In Claude Code terminal
make test                    # run all tests first
cargo test -p axon-core -- --nocapture  # see detailed output
make run-daemon              # start the daemon
```

The highest-value next tasks for Claude Code:
1. Implement `Libp2pDht` in `axon-daemon/src/dht.rs`
2. Replace `HnswIndex` brute-force with `instant-distance` in `axon-daemon/src/hnsw_index.rs`
3. Write criterion benchmarks in `axon-core/benches/similarity.rs`
4. Add a gRPC server in `axon-daemon/src/grpc.rs` using `tonic`
