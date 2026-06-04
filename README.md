# AXON Protocol

[![CI](https://github.com/samabdelkhalek-code/axon-protocol/actions/workflows/ci.yml/badge.svg)](https://github.com/samabdelkhalek-code/axon-protocol/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange)](https://rustup.rs)
[![SUI Move](https://img.shields.io/badge/Move-SUI-6fbcf0)](https://docs.sui.io)

> **The TCP/IP layer for autonomous AI agents.**
> Zero-trust discovery, cryptographic capability verification, and atomic micro-settlement — all headless, all at machine speed.

---

## What this is

AXON is a decentralised infrastructure protocol that lets autonomous agents find each other, prove their capabilities, and pay each other — with no humans, no central servers, and no trusted intermediaries.

When Agent A needs a specialised sub-task done by Agent B, AXON handles:

1. **Discovery** — semantic vector search over a Kademlia DHT to find the right agent in <50ms
2. **Verification** — zero-knowledge proof that B's declared capabilities match its actual system prompt
3. **Settlement** — atomic escrow on SUI Move that releases payment only when B submits cryptographic proof of task acknowledgment

**This is not a SaaS, a marketplace, or an agent framework.** It is the protocol layer that agent frameworks talk to.

---

## Repository layout

```
axon-protocol/
├── axon-core/          # Protocol types, handshake engine, crypto primitives
│   ├── src/
│   │   ├── constants.rs   ← single source of truth for all protocol parameters
│   │   ├── errors.rs      ← typed error hierarchy
│   │   ├── manifest.rs    ← AgentManifest + ManifestBuilder
│   │   ├── handshake.rs   ← HandshakeEngine (deterministic, no I/O)
│   │   ├── similarity.rs  ← SIMD-ready cosine similarity + composite ranking
│   │   └── settlement.rs  ← SessionRecord, PriceOracle
│   └── tests/
│       └── integration.rs ← end-to-end handshake lifecycle tests
│
├── axon-daemon/        # axond binary — DHT node, router, escrow monitor
│   └── src/
│       ├── main.rs        ← entry point, config loading, orchestration
│       ├── config.rs      ← CLI flags + env vars (clap)
│       ├── dht.rs         ← Dht trait + InProcessDht stub
│       └── hnsw_index.rs  ← HnswIndex (brute-force MVP → HNSW in prod)
│
├── axon-sdk/           # One-call developer SDK
│   └── src/
│       ├── lib.rs
│       └── register.rs    ← register() — key gen, embedding, manifest publish
│
├── axon-cli/           # `axon` CLI tool
│
├── contracts/
│   └── axon/
│       └── sources/
│           └── settlement.move  ← AgentEscrow, AgentReputation, settlement logic
│
├── proto/              # gRPC service definitions (tonic / prost)
│   ├── axon_manifest.proto
│   └── axon_handshake.proto
│
├── scripts/
│   ├── dev_setup.sh    ← first-run environment setup
│   └── deploy_local.sh ← deploy Move contracts to local SUI node
│
└── docker/
    ├── Dockerfile
    └── docker-compose.yml
```

---

## Quickstart

### Prerequisites

- **Rust 1.78+** — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **SUI CLI** — `cargo install --locked --git https://github.com/MystenLabs/sui.git sui`

### 1. Clone and setup

```bash
git clone https://github.com/samabdelkhalek-code/axon-protocol.git
cd axon-protocol
./scripts/dev_setup.sh
cp .env.example .env
```

### 2. Run the test suite

```bash
make test
```

### 3. Start the daemon (devnet mode)

```bash
make run-daemon
```

### 4. Register an agent via the SDK

```rust
use axon_sdk::{register, AXON_DEVNET_GENESIS};

let result = register(
    "Summarises long documents into structured bullet points with citations",
    1_000,       // 1000 picoSUI per compute unit
    50_000_000,  // 50M picoSUI stake bond
    &AXON_DEVNET_GENESIS,
).await?;
```

### 5. Deploy the settlement contract to SUI devnet

```bash
sui client switch --env devnet
sui client faucet
make deploy-devnet
```

---

## License

MIT — see [LICENSE](LICENSE).
