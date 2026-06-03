#!/usr/bin/env bash
# AXON Protocol — development environment setup
# Run once after cloning the repo.
set -euo pipefail

BOLD="\033[1m"; CYAN="\033[0;36m"; GREEN="\033[0;32m"; RESET="\033[0m"

header() { echo -e "\n${BOLD}${CYAN}▶ $1${RESET}"; }
ok()     { echo -e "  ${GREEN}✓${RESET} $1"; }

header "Checking Rust toolchain"
if ! command -v rustup &>/dev/null; then
  echo "Installing rustup…"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi
rustup component add clippy rustfmt
ok "Rust stable + clippy + rustfmt"

header "Installing cargo tools"
cargo install cargo-nextest --locked 2>/dev/null && ok "cargo-nextest" || echo "  (skipped cargo-nextest)"
cargo install cargo-audit   --locked 2>/dev/null && ok "cargo-audit"   || echo "  (skipped cargo-audit)"

header "SUI CLI"
if command -v sui &>/dev/null; then
  ok "SUI CLI already installed: $(sui --version)"
else
  echo "  SUI CLI not found. Install manually:"
  echo "  cargo install --locked --git https://github.com/MystenLabs/sui.git sui"
  echo "  Or download from: https://docs.sui.io/guides/developer/getting-started/sui-install"
fi

header "Creating dev directories"
mkdir -p .axon
ok ".axon/ created (key + DB will be stored here)"

header "Copying .env.example"
if [ ! -f .env ]; then
  cp .env.example .env
  ok ".env created from .env.example"
else
  echo "  .env already exists — skipping"
fi

echo -e "\n${BOLD}${GREEN}Dev environment ready.${RESET}"
echo "  make test        — run all tests"
echo "  make run-daemon  — start axond"
echo "  make deploy-devnet — deploy Move contracts to SUI devnet"
