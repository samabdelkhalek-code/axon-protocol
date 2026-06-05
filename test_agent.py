#!/usr/bin/env python3
"""
AXON Test Agent
===============
Demonstrates the full agent-to-agent flow:
  1. Discover invoice agent via axond
  2. Send HandshakeRequest
  3. Call /v1/agent-task (create invoice)
  4. Print result + compute units used
  5. TODO: settle payment on SUI (next step)

Usage:
  python test_agent.py
"""

import os
import time
import uuid
import hashlib
import struct
import json
import blake3
import requests
import grpc

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives import serialization

# Proto imports (generated in repo root)
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import axon_manifest_pb2 as manifest_pb2
import axon_manifest_pb2_grpc as manifest_grpc
import axon_handshake_pb2 as handshake_pb2
import axon_handshake_pb2_grpc as handshake_grpc

AXON_GRPC        = os.getenv("AXON_GRPC_ADDR", "axon.alpen-huettentouren.de:50051")
INVOICE_AGENT_URL = os.getenv("INVOICE_AGENT_URL", "https://invoice.alpen-huettentouren.de")
GENESIS_HASH     = bytes(32)  # devnet placeholder
EMBEDDING_DIM    = 768


def generate_key() -> Ed25519PrivateKey:
    return Ed25519PrivateKey.generate()


def pub_bytes(key: Ed25519PrivateKey) -> bytes:
    return key.public_key().public_bytes(
        serialization.Encoding.Raw,
        serialization.PublicFormat.Raw,
    )


def make_embedding(text: str) -> bytes:
    seed = blake3.blake3(text.encode()).digest()
    return bytes([(seed[i % len(seed)] if seed[i % len(seed)] < 128
                   else seed[i % len(seed)] - 256) & 0xFF
                  for i in range(EMBEDDING_DIM)])


def step(n, msg):
    print(f"\n{'='*60}")
    print(f"  STEP {n}: {msg}")
    print(f"{'='*60}")


# ── Step 1: Connect to axond ─────────────────────────────────────────────────

step(1, "Connect to axond")
channel = grpc.insecure_channel(AXON_GRPC)
discovery_stub  = manifest_grpc.DiscoveryServiceStub(channel)
handshake_stub  = handshake_grpc.HandshakeServiceStub(channel)
print(f"  Connected to {AXON_GRPC}")


# ── Step 2: Discover invoice agent ───────────────────────────────────────────

step(2, "Discover invoice agent")
query_emb = make_embedding("create invoice rechnung PDF §14 UStG german")
resp = discovery_stub.Discover(
    manifest_pb2.DiscoverRequest(requirement_embedding=query_emb, top_k=5),
    timeout=10,
)
print(f"  Found {len(resp.results)} agent(s)")

invoice_agent = None
for r in resp.results:
    tags = list(r.manifest.capability_tags)
    print(f"  → tags={tags}  price={r.manifest.base_price_per_cu} picoSUI/CU")
    if "invoice" in tags or "rechnung" in tags:
        invoice_agent = r.manifest

if not invoice_agent:
    print("  ✗ No invoice agent found — is it registered?")
    sys.exit(1)

print(f"\n  ✓ Invoice agent found!")
print(f"    agent_id : {invoice_agent.agent_id.hex()[:16]}...")
print(f"    price    : {invoice_agent.base_price_per_cu} picoSUI/CU")


# ── Step 3: Handshake ─────────────────────────────────────────────────────────

step(3, "Send HandshakeRequest")
initiator_key = generate_key()
initiator_pub  = pub_bytes(initiator_key)
initiator_id   = blake3.blake3(initiator_pub + GENESIS_HASH).digest()

session_id     = uuid.uuid4().bytes
task_payload   = json.dumps({
    "customer": {"name": "Test GmbH", "city": "Berlin"},
    "items": [{"description": "AXON Agent Test", "quantity": 1, "unit_price": 100.0}],
}).encode()
task_hash      = blake3.blake3(task_payload).digest()

max_cu         = 20
price_per_cu   = invoice_agent.base_price_per_cu
escrow_amount  = max_cu * price_per_cu
deadline_ns    = int((time.time() + 3600) * 1e9)

# Build signature payload matching HandshakeRequest::content_hash() in Rust
sig_payload = (
    session_id +
    initiator_id +
    invoice_agent.agent_id +
    task_hash +
    struct.pack("<Q", max_cu) +
    struct.pack("<Q", price_per_cu) +
    struct.pack("<Q", escrow_amount) +
    bytes(32) +  # escrow_object_id placeholder
    deadline_ns.to_bytes(16, "little")
)
sig = initiator_key.sign(sig_payload)

handshake_req = handshake_pb2.HandshakeRequest(
    session_id                    = session_id,
    initiator_id                  = initiator_id,
    target_id                     = invoice_agent.agent_id,
    required_capability_embedding = query_emb,
    task_payload_hash             = task_hash,
    max_compute_units             = max_cu,
    max_price_per_cu              = price_per_cu,
    escrow_amount                 = escrow_amount,
    escrow_object_id              = bytes(32),
    deadline_ns                   = deadline_ns,
    initiator_signature           = sig,
)

try:
    hs_resp = handshake_stub.Handshake(handshake_req, timeout=10)
    status_name = handshake_pb2.HandshakeStatus.Name(hs_resp.status)
    print(f"  Handshake status : {status_name}")
    print(f"  Agreed price/CU  : {hs_resp.agreed_price_per_cu} picoSUI")
    print(f"  Commitment hash  : {hs_resp.commitment_hash.hex()[:16]}...")
except grpc.RpcError as e:
    print(f"  Handshake gRPC error: {e.code()} — {e.details()}")
    print("  (Continuing with task call anyway — handshake is optional for MVP)")


# ── Step 4: Call agent task ───────────────────────────────────────────────────

step(4, "Call /v1/agent-task")
payload = {
    "session_id": session_id.hex(),
    "action": "create_invoice",
    "params": {
        "customer": {
            "name": "Test GmbH",
            "company": "Test GmbH",
            "street": "Teststraße 1",
            "zip": "10115",
            "city": "Berlin",
            "email": "test@test.de",
        },
        "items": [{
            "description": "AXON Agent Test — automatisch erstellte Rechnung",
            "quantity": 1,
            "unit": "Pauschal",
            "unit_price": 100.0,
            "vat_rate": 19.0,
        }],
        "notes": f"Erstellt durch AXON Test Agent | Session: {session_id.hex()[:8]}",
    },
}

r = requests.post(f"{INVOICE_AGENT_URL}/v1/agent-task", json=payload, timeout=30)
if r.status_code == 200:
    result = r.json()
    print(f"  ✓ Invoice created!")
    print(f"    Invoice number : {result['result'].get('invoice_number', '?')}")
    print(f"    Compute units  : {result['compute_units']} CU")
    print(f"    Cost           : {result['compute_units'] * price_per_cu} picoSUI")
    print(f"    Elapsed        : {result['elapsed_ms']:.1f}ms")
else:
    print(f"  ✗ Task failed: {r.status_code} — {r.text[:200]}")


# ── Step 5: Settlement (TODO) ─────────────────────────────────────────────────

step(5, "Settlement (SUI on-chain)")
cost_pico = result['compute_units'] * price_per_cu if r.status_code == 200 else 0
cost_sui  = cost_pico / 1_000_000_000_000

print(f"""
  Amount due : {cost_pico} picoSUI ({cost_sui:.12f} SUI)
  Contract   : 0xa0e08b5372c8aef3fbc8381ff1be2ddaaba0eaeb82a547d5124b92d58c295293
  Network    : SUI Devnet

  TODO: Call settle_escrow() on-chain:
    sui client call --package 0xa0e08b53... \\
      --module settlement --function settle_escrow \\
      --args <escrow_obj> <preimage> {result['compute_units']} {price_per_cu}

  → This is the next implementation step.
""")

print("✅ End-to-end flow complete (minus on-chain settlement)!")
