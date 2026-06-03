# AXON Protocol — Makefile
# Run `make help` to see all available commands.

SHELL        := /bin/bash
CARGO        := cargo
SUI          := sui
RUSTFLAGS_OPT := RUSTFLAGS="-C target-cpu=native"

.DEFAULT_GOAL := help

.PHONY: help build test lint fmt clean dev-setup \
        deploy-devnet deploy-testnet run-daemon \
        bench doc check-all

help: ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | \
	  awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}'

# ── Rust ──────────────────────────────────────────────────────────────────────

build: ## Build all workspace crates (debug)
	$(CARGO) build --workspace

build-release: ## Build all workspace crates (release + CPU optimisations)
	$(RUSTFLAGS_OPT) $(CARGO) build --workspace --release

test: ## Run all unit and integration tests
	$(CARGO) test --workspace -- --nocapture

test-core: ## Run only axon-core tests
	$(CARGO) test -p axon-core -- --nocapture

bench: ## Run criterion benchmarks (axon-core)
	$(RUSTFLAGS_OPT) $(CARGO) bench -p axon-core

lint: ## Run clippy (deny warnings)
	$(CARGO) clippy --workspace -- -D warnings

fmt: ## Format all Rust code
	$(CARGO) fmt --all

fmt-check: ## Check formatting without modifying files
	$(CARGO) fmt --all -- --check

doc: ## Build and open documentation
	$(CARGO) doc --workspace --no-deps --open

clean: ## Remove all build artefacts
	$(CARGO) clean

check-all: fmt-check lint test ## fmt + lint + test (CI equivalent)

# ── Daemon ────────────────────────────────────────────────────────────────────

run-daemon: ## Start axond with dev defaults
	AXON_LOG_LEVEL=debug $(CARGO) run -p axon-daemon -- \
	  --listen-addr /ip4/127.0.0.1/tcp/7777 \
	  --sui-rpc https://fullnode.devnet.sui.io:443

run-daemon-release: build-release ## Start axond (release build)
	./target/release/axond

# ── SUI / Move ───────────────────────────────────────────────────────────────

deploy-devnet: ## Deploy settlement contract to SUI devnet
	@echo "Deploying to SUI devnet…"
	$(SUI) client publish --gas-budget 100000000 contracts/axon

deploy-testnet: ## Deploy settlement contract to SUI testnet
	@echo "Deploying to SUI testnet…"
	$(SUI) client switch --env testnet
	$(SUI) client publish --gas-budget 100000000 contracts/axon

move-test: ## Run Move unit tests
	$(SUI) move test --path contracts/axon

move-build: ## Compile Move contracts (check for errors)
	$(SUI) move build --path contracts/axon

# ── Dev environment ───────────────────────────────────────────────────────────

dev-setup: ## Install required tools (Rust nightly, SUI, protoc)
	@echo "Installing Rust toolchain components…"
	rustup component add clippy rustfmt
	@echo "Installing cargo tools…"
	cargo install cargo-nextest --locked 2>/dev/null || true
	@echo ""
	@echo "Install SUI CLI manually if not present:"
	@echo "  cargo install --locked --git https://github.com/MystenLabs/sui.git sui"
	@echo ""
	@echo "Install protoc if using gRPC:"
	@echo "  apt-get install -y protobuf-compiler   (Ubuntu)"
	@echo "  brew install protobuf                  (macOS)"

# ── Docker ────────────────────────────────────────────────────────────────────

docker-build: ## Build the axond Docker image
	docker build -f docker/Dockerfile -t axon-daemon:latest .

docker-up: ## Start local axond node via docker-compose
	docker compose -f docker/docker-compose.yml up -d

docker-down: ## Stop local docker stack
	docker compose -f docker/docker-compose.yml down

docker-logs: ## Tail daemon logs
	docker compose -f docker/docker-compose.yml logs -f axond
