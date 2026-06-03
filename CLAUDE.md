# CLAUDE.md — AXON Protocol

---

## ⛔ ABSOLUTES NO-PUSH — LOKAL ONLY

**Dieses Projekt bleibt ausschliesslich lokal auf dem Rechner von Säm Abdelkhalek.**

- **Kein `git push`** — weder zu `origin` noch zu irgendeinem anderen Remote
- **Kein Deployment** auf Server, Cloud, CI/CD oder sonstige Infrastruktur
- **Kein Teilen** von Code, Keys oder Credentials ausserhalb dieses Rechners
- **Keine GitHub Actions** werden ausgelöst — da nie gepusht wird, läuft `ci.yml` nie

Ein pre-push Git Hook blockiert jeden versehentlichen Push automatisch.
Remote `origin` bleibt konfiguriert, wird aber **nie** genutzt.

Wenn das Projekt irgendwann veröffentlicht werden soll → neues Repository,
neuer Plan, explizite Entscheidung.

---

## Projekt-Übersicht

AXON ist ein dezentrales Infrastruktur-Protokoll für autonome AI-Agenten:
- **Discovery** — semantische Vektorsuche über Kademlia DHT (<50ms)
- **Verification** — Zero-Knowledge-Proof für Capability-Verifikation
- **Settlement** — atomares Escrow auf SUI Move

Stack: **Rust 1.78+** + **SUI Move**

---

## Lokaler Setup

```bash
cargo build --workspace          # alles bauen
cargo test --workspace           # tests
cargo clippy --workspace         # linter
make dev                         # development mode (siehe Makefile)
```

---

## Struktur

```
axon-protocol/
├── axon-core/      — Kern-Protokoll (Rust)
├── axon-cli/       — CLI-Tool
├── axon-daemon/    — Daemon-Prozess
├── axon-sdk/       — SDK
├── contracts/      — SUI Move Smart Contracts
├── proto/          — Protobuf Definitionen
├── docker/         — lokale Docker-Umgebung
├── docs/           — Dokumentation
└── scripts/        — Hilfsskripte
```

---

## Sicherheit

- `.axon/identity.key` — niemals committen (in .gitignore)
- `.env` / `.env.local` — niemals committen
- `contracts/axon/build/` — nicht committen
