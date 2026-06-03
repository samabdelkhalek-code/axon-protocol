# Contributing to AXON Protocol

## Development workflow

```bash
git clone https://github.com/your-org/axon-protocol
cd axon-protocol
./scripts/dev_setup.sh
```

All contributions go through a pull request against `main`.

## Before submitting a PR

```bash
make check-all   # fmt-check + lint + test — same as CI
```

## Code standards

- No `unwrap()` or `expect()` in library code (`axon-core`, `axon-sdk`)
- Every public function must have a doc comment
- New logic = new unit test in the same file
- Protocol parameter changes go in `axon-core/src/constants.rs` only

## Priority areas

| Area | File | Skill needed |
|------|------|------|
| libp2p DHT | `axon-daemon/src/dht.rs` | Rust async, libp2p |
| HNSW index | `axon-daemon/src/hnsw_index.rs` | Rust, ANN algorithms |
| ZK circuit | `contracts/circuit/` (create) | Circom, Groth16 |
| Benchmarks | `axon-core/benches/` (create) | criterion.rs |
| gRPC layer | `axon-daemon/src/grpc.rs` (create) | tonic, prost |

## Commit message format

```
type(scope): short description

body (optional)

Refs: #issue
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`
