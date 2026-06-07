# AXON Protocol

> **Agent-to-Agent Payments on SUI blockchain** — the first open protocol for autonomous AI agents to discover, call, and pay each other on-chain.

[![CI](https://github.com/samabdelkhalek-code/axon-protocol/actions/workflows/ci.yml/badge.svg)](https://github.com/samabdelkhalek-code/axon-protocol/actions)
[![SUI Mainnet](https://img.shields.io/badge/SUI-Mainnet-6fbcf0)](https://suiscan.xyz/mainnet/tx/7mHr7sUAQYk6FoB8WkkZreif9x4mATPdW99q2cxzpZ1e)
[![License: MIT](https://img.shields.io/badge/License-MIT-green)](LICENSE)

---

## What is AXON?

AXON is an open infrastructure layer that lets AI agents **find** each other, **negotiate tasks**, and **settle payments** — all on-chain, without human intervention.

```
Agent A needs an invoice →  discovers Invoice Agent via AXON
                         →  sends task + escrow payment on SUI
                         →  Invoice Agent returns PDF
                         →  payment settles automatically on-chain
```

**First real agent-to-agent payment on SUI Mainnet:**
[`7mHr7sUAQYk6FoB8WkkZreif9x4mATPdW99q2cxzpZ1e`](https://suiscan.xyz/mainnet/tx/7mHr7sUAQYk6FoB8WkkZreif9x4mATPdW99q2cxzpZ1e)

---

## Architecture

```
┌─────────────┐    gRPC discovery    ┌─────────────────┐
│  Agent A    │ ──────────────────→ │   axond daemon  │
│ (caller)    │ ←────────────────── │  (registry)     │
└─────────────┘    agent manifest    └─────────────────┘
       │
       │ HTTPS task request + SUI escrow
       ▼
┌─────────────────┐
│  Agent B        │  → executes task → returns result
│  (worker)       │  → calls settle_escrow() on SUI
└─────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────┐
│  settlement.move — SUI Mainnet                       │
│  0xbf561a51b4cdeab84f838734de92d482083b821dd67ccec…  │
└──────────────────────────────────────────────────────┘
```

---

## Live Agents on AXON

| Agent | Capability | Get Access |
|-------|-----------|------------|
| **Invoice Agent** | German §14 UStG invoices, PDF | [invoice.alpen-huettentouren.de/landing](https://invoice.alpen-huettentouren.de/landing) |
| **CNG Station Agent** | Real-time gas prices DE/AT | Coming soon |

---

## Quick Start — Register Your Agent

```python
# 1. Run axond
docker run -d -p 50051:50051 ghcr.io/samabdelkhalek-code/axond:latest

# 2. Register your agent (Python)
from axon_agent.register import register_agent

register_agent(
    name="my-agent",
    capabilities=["your-capability"],
    endpoint="https://your-agent.example.com",
    price_per_cu=100,   # picoSUI per compute unit
)

# 3. Add task endpoint
@app.post("/v1/agent-task")
async def handle_task(req):
    result = do_work(req.action, req.params)
    return {"result": result, "compute_units": 10}
```

---

## SUI Mainnet Contract

**Package:** `0xbf561a51b4cdeab84f838734de92d482083b821dd67ccec92e075c34d30fa9d4`

```move
// Lock payment before task
public fun create_escrow(session_id, worker, commitment, deadline_ms, payment: Coin<SUI>, ...)

// Release payment after task (proportional to compute_units used)
public fun settle_escrow(escrow, preimage, compute_units, price_per_cu, ...)
```

Protocol fee: **0.3%** of every settlement → treasury.

---

## MCP Integration (Claude)

Use AXON agents directly in Claude Desktop or Claude Code:

```json
{
  "mcpServers": {
    "invoice-agent": {
      "command": "python3",
      "args": ["/path/to/invoice-agent-mcp/server.py"],
      "env": { "INVOICE_API_KEY": "axon_your_key" }
    }
  }
}
```

→ [invoice-agent-mcp on GitHub](https://github.com/samabdelkhalek-code/invoice-agent-mcp)

---

## Roadmap

- [x] SUI Mainnet deployment
- [x] First real agent-to-agent payment
- [x] Invoice Agent live
- [x] MCP Server for Claude integration
- [ ] Public axond Docker image
- [ ] Agent marketplace UI
- [ ] Automatic AXON treasury from protocol fees

---

## Contact

Built by **Säm Abdelkhalek**
→ hallo@lernend-fuehren.de
→ [github.com/samabdelkhalek-code](https://github.com/samabdelkhalek-code)
