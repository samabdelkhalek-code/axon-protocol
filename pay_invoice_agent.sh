#!/bin/bash
# ── AXON On-Chain Payment Demo ────────────────────────────────────────────────
# Full flow: create_reputation → create_escrow → task → settle_escrow
# Requires: sui CLI, devnet balance

set -e
export PATH="/usr/local/bin:$PATH"

PACKAGE="0x6fb5c1c5763e584da53914dc7b2c702afdde527f20e3f003f302eeaec29f2a31"
CLOCK="0x6"
MY_ADDR=$(sui client active-address)
TREASURY="$MY_ADDR"   # treasury = our own address for devnet demo
RESPONDER="$MY_ADDR"  # invoice agent uses same address for demo

echo "=== AXON On-Chain Payment Demo ==="
echo "Package : $PACKAGE"
echo "Address : $MY_ADDR"
echo ""

# ── Step 1: Create AgentReputation ───────────────────────────────────────────
echo "STEP 1: Create AgentReputation..."
REP_TX=$(sui client call \
  --package "$PACKAGE" \
  --module settlement \
  --function register_agent \
  --gas-budget 10000000 \
  --json 2>/dev/null)

REP_ID=$(echo "$REP_TX" | python3 -c "
import json, sys
tx = json.load(sys.stdin)
for obj in tx.get('objectChanges', []):
    if obj.get('objectType','').endswith('AgentReputation'):
        print(obj['objectId'])
        break
")

if [ -z "$REP_ID" ]; then
  echo "✗ Failed to get reputation object ID"
  echo "$REP_TX" | python3 -m json.tool 2>/dev/null | head -40
  exit 1
fi

echo "✓ AgentReputation created & shared: $REP_ID"

# ── Step 2: Create Escrow ─────────────────────────────────────────────────────
echo ""
echo "STEP 2: Create Escrow..."

SESSION_ID="0x$(python3 -c "import uuid; print(uuid.uuid4().hex)")"
PREIMAGE="0x$(python3 -c "import secrets; print(secrets.token_hex(32))")"
COMMITMENT="0x$(python3 -c "
import hashlib, sys
preimage = bytes.fromhex('${PREIMAGE#0x}')
print(hashlib.sha256(preimage).hexdigest())
")"
DEADLINE_MS=$(python3 -c "import time; print(int((time.time() + 3600) * 1000))")
PAYMENT_PICO=5000  # 5000 picoSUI = 0.000000005 SUI

echo "Session  : $SESSION_ID"
echo "Payment  : $PAYMENT_PICO picoSUI"
echo "Deadline : +1 hour"

ESCROW_TX=$(sui client call \
  --package "$PACKAGE" \
  --module settlement \
  --function create_escrow \
  --args \
    "$SESSION_ID" \
    "$RESPONDER" \
    "$COMMITMENT" \
    "$DEADLINE_MS" \
    "$TREASURY" \
    "$PAYMENT_PICO" \
    "$CLOCK" \
  --gas-budget 10000000 \
  --json 2>/dev/null)

ESCROW_ID=$(echo "$ESCROW_TX" | python3 -c "
import json, sys
tx = json.load(sys.stdin)
for obj in tx.get('objectChanges', []):
    if obj.get('objectType','').endswith('AgentEscrow'):
        print(obj['objectId'])
        break
")

if [ -z "$ESCROW_ID" ]; then
  echo "✗ Failed to create escrow"
  echo "$ESCROW_TX" | python3 -m json.tool 2>/dev/null | head -40
  exit 1
fi

echo "✓ Escrow created: $ESCROW_ID"

# ── Step 3: Call Invoice Agent Task ──────────────────────────────────────────
echo ""
echo "STEP 3: Call Invoice Agent..."

TASK_RESULT=$(curl -s -X POST https://invoice.alpen-huettentouren.de/v1/agent-task \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"${SESSION_ID}\",
    \"action\": \"create_invoice\",
    \"params\": {
      \"customer\": {\"name\": \"AXON Payment Test GmbH\", \"city\": \"Berlin\", \"street\": \"Teststr. 1\", \"zip\": \"10115\"},
      \"items\": [{\"description\": \"AXON On-Chain Payment Demo\", \"quantity\": 1, \"unit\": \"Pauschal\", \"unit_price\": 500.0, \"vat_rate\": 19.0}],
      \"notes\": \"Escrow: ${ESCROW_ID}\"
    }
  }")

INVOICE_NUM=$(echo "$TASK_RESULT" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result'].get('invoice_number','?'))")
COMPUTE_UNITS=$(echo "$TASK_RESULT" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['compute_units'])")
echo "✓ Invoice: $INVOICE_NUM | CU used: $COMPUTE_UNITS"

# ── Step 4: Settle Escrow ─────────────────────────────────────────────────────
echo ""
echo "STEP 4: Settle Escrow on-chain..."

sui client call \
  --package "$PACKAGE" \
  --module settlement \
  --function settle_escrow \
  --args \
    "$ESCROW_ID" \
    "$PREIMAGE" \
    "$COMPUTE_UNITS" \
    "500" \
    "$REP_ID" \
    "$CLOCK" \
  --gas-budget 10000000

echo ""
echo "============================================"
echo "✅ FIRST AXON ON-CHAIN PAYMENT COMPLETE!"
echo "   Invoice : $INVOICE_NUM"
echo "   Escrow  : $ESCROW_ID"
echo "   CU used : $COMPUTE_UNITS"
echo "   Paid    : $PAYMENT_PICO picoSUI"
echo "   Network : SUI Devnet"
echo "   Explorer: https://suiexplorer.com/object/$ESCROW_ID?network=devnet"
echo "============================================"
