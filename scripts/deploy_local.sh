#!/usr/bin/env bash
# Deploy the AXON settlement contract to a local SUI node.
# Prerequisites: SUI CLI installed, `sui start` running in another terminal.
set -euo pipefail

echo "Switching to localnet…"
sui client switch --env localnet

echo "Requesting devnet faucet tokens…"
sui client faucet || true

echo "Publishing axon::settlement…"
RESULT=$(sui client publish \
  --gas-budget 100000000 \
  contracts/axon \
  --json)

echo "$RESULT" | python3 -c "
import json, sys
data = json.load(sys.stdin)
pkg = next(
  obj['packageId']
  for obj in data['objectChanges']
  if obj.get('type') == 'published'
)
print(f'Package ID: {pkg}')
print(f'Add to .env: AXON_PACKAGE_ID={pkg}')
" 2>/dev/null || echo "Package published. Check output above for package ID."
