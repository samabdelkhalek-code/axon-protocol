//! AgentManifest — signed, content-addressed capability declaration.
//!
//! Every agent in the AXON network publishes one manifest to the Kademlia DHT
//! on startup. The manifest is immutable once signed; updates require a new
//! manifest with a higher `timestamp_ns` and a fresh signature.

use blake3::Hasher;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::constants::EMBEDDING_DIM;
use crate::errors::AxonError;

/// Maximum number of capability tags per manifest.
pub const MAX_CAPABILITY_TAGS: usize = 16;
/// Maximum byte length of a single capability tag.
pub const MAX_TAG_LENGTH: usize = 64;

/// Signed, content-addressed declaration of an agent's capabilities and pricing.
///
/// # Content addressing
/// DHT storage key = `BLAKE3(all fields except the `signature` field)`.
/// This is also the message that the Ed25519 signature covers.
///
/// # Embedding derivation
/// `capability_embedding` must be derived by running the agent's system prompt
/// through the canonical sentence-transformer model (weights CID pinned at
/// AXON genesis block). A Groth16 ZK proof (`zk_capability_proof`) attests to
/// this derivation without revealing the system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    /// Agent identifier: `BLAKE3(public_key_bytes || genesis_block_hash)`.
    /// Ties the identity to a specific AXON network epoch.
    pub agent_id: [u8; 32],

    /// Ed25519 verifying key bytes used to verify all subsequent messages
    /// from this agent, including HandshakeRequests and HandshakeResponses.
    pub public_key: [u8; 32],

    /// Semantic capability vector: 768-dimensional, int8-quantized.
    /// Quantized from float32 at registration time (4× size reduction).
    /// Must have exactly `EMBEDDING_DIM` elements.
    pub capability_embedding: Vec<i8>,

    /// Human-readable capability descriptors for the DHT keyword index.
    /// At most `MAX_CAPABILITY_TAGS` tags, each at most `MAX_TAG_LENGTH` bytes.
    pub capability_tags: Vec<String>,

    /// Declared p95 execution latency SLA in microseconds.
    /// Used as a tie-breaker in ANN ranking; persistent over-runs penalise score.
    pub latency_sla_us: u64,

    /// Base price per Compute Unit (picoSUI).
    /// 1 CU = 1 ms of single-core CPU at the AXON reference hardware spec.
    pub base_price_per_cu: u64,

    /// On-chain SUI stake locked as a performance bond (picoSUI).
    /// Protocol requires `staked_amount >= 2 × max_expected_escrow`.
    pub staked_amount: u64,

    /// Groth16 proof (128 bytes, BN254 curve) that `capability_embedding` was
    /// produced by the canonical embedding model applied to this agent's system
    /// prompt. Verified on-chain at registration.
    pub zk_capability_proof: Vec<u8>,

    /// Manifest creation timestamp (Unix nanoseconds, monotonically increasing).
    /// Higher timestamp wins on DHT key conflicts from the same `agent_id`.
    pub timestamp_ns: u128,

    /// Ed25519 signature over `self.content_hash()`.
    /// Must be verified before any trust decision is made on this manifest.
    pub signature: Vec<u8>,
}

impl AgentManifest {
    /// Canonical BLAKE3 hash of all fields except `signature`.
    ///
    /// Used as:
    /// - DHT storage key
    /// - Message covered by the Ed25519 signature
    /// - Input to ZK proof verification
    pub fn content_hash(&self) -> [u8; 32] {
        let mut h = Hasher::new();
        h.update(&self.agent_id);
        h.update(&self.public_key);

        // i8 and u8 have identical bit representation — cast is semantically correct.
        let emb_bytes: Vec<u8> = self.capability_embedding
            .iter()
            .map(|&x| x as u8)
            .collect();
        h.update(&emb_bytes);

        for tag in &self.capability_tags {
            h.update(tag.as_bytes());
        }
        h.update(&self.latency_sla_us.to_le_bytes());
        h.update(&self.base_price_per_cu.to_le_bytes());
        h.update(&self.staked_amount.to_le_bytes());
        h.update(&self.zk_capability_proof);
        h.update(&self.timestamp_ns.to_le_bytes());

        *h.finalize().as_bytes()
    }

    /// Verify the Ed25519 signature.
    ///
    /// Must be called before any routing or trust decision based on this manifest.
    /// Returns `Ok(())` if the signature is valid, `Err` otherwise.
    pub fn verify_signature(&self) -> Result<(), AxonError> {
        let vk = VerifyingKey::from_bytes(&self.public_key)
            .map_err(|_| AxonError::InvalidPublicKey)?;

        let sig = Signature::from_slice(&self.signature)
            .map_err(|_| AxonError::InvalidSignature)?;

        vk.verify_strict(&self.content_hash(), &sig)
            .map_err(|_| AxonError::SignatureVerificationFailed)
    }

    /// Validate structural invariants (dimensions, tag lengths, etc.).
    ///
    /// Call before publishing or storing a manifest.
    pub fn validate_structure(&self) -> Result<(), AxonError> {
        if self.capability_embedding.len() != EMBEDDING_DIM {
            return Err(AxonError::EmbeddingDimensionMismatch {
                expected: EMBEDDING_DIM,
                actual: self.capability_embedding.len(),
            });
        }
        if self.capability_tags.len() > MAX_CAPABILITY_TAGS {
            return Err(AxonError::TooManyCapabilityTags {
                count: self.capability_tags.len(),
                max: MAX_CAPABILITY_TAGS,
            });
        }
        for tag in &self.capability_tags {
            if tag.len() > MAX_TAG_LENGTH {
                return Err(AxonError::CapabilityTagTooLong {
                    len: tag.len(),
                    max: MAX_TAG_LENGTH,
                });
            }
        }
        Ok(())
    }

    /// Returns the hex-encoded agent_id (short form for logging).
    pub fn short_id(&self) -> String {
        hex::encode(&self.agent_id[..8])
    }
}

// ── Builder (for testing and SDK use) ─────────────────────────────────────────

/// Helper to construct a signed `AgentManifest` from a `SigningKey`.
///
/// For production use, the SDK calls the local embedding service and the
/// ZK prover before invoking `build()`.
pub struct ManifestBuilder {
    agent_id:             Option<[u8; 32]>,
    capability_embedding: Option<Vec<i8>>,
    capability_tags:      Vec<String>,
    latency_sla_us:       u64,
    base_price_per_cu:    u64,
    staked_amount:        u64,
    zk_capability_proof:  Vec<u8>,
    timestamp_ns:         Option<u128>,
}

impl ManifestBuilder {
    pub fn new() -> Self {
        Self {
            agent_id: None,
            capability_embedding: None,
            capability_tags: Vec::new(),
            latency_sla_us: 0,
            base_price_per_cu: 0,
            staked_amount: 0,
            zk_capability_proof: Vec::new(),
            timestamp_ns: None,
        }
    }

    pub fn agent_id(mut self, id: [u8; 32]) -> Self {
        self.agent_id = Some(id); self
    }
    pub fn embedding(mut self, emb: Vec<i8>) -> Self {
        self.capability_embedding = Some(emb); self
    }
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.capability_tags = tags; self
    }
    pub fn latency_sla_us(mut self, us: u64) -> Self {
        self.latency_sla_us = us; self
    }
    pub fn price_per_cu(mut self, price: u64) -> Self {
        self.base_price_per_cu = price; self
    }
    pub fn stake(mut self, amount: u64) -> Self {
        self.staked_amount = amount; self
    }
    pub fn zk_proof(mut self, proof: Vec<u8>) -> Self {
        self.zk_capability_proof = proof; self
    }

    /// Sign and build the manifest.
    pub fn build(
        self,
        signing_key: &ed25519_dalek::SigningKey,
        genesis_block_hash: &[u8; 32],
    ) -> Result<AgentManifest, AxonError> {
        use ed25519_dalek::Signer;
        use std::time::{SystemTime, UNIX_EPOCH};

        let public_key_bytes = signing_key.verifying_key().to_bytes();

        let agent_id = *Hasher::new()
            .update(&public_key_bytes)
            .update(genesis_block_hash)
            .finalize()
            .as_bytes();

        let timestamp_ns = self.timestamp_ns.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Clock regression")
                .as_nanos()
        });

        let embedding = self.capability_embedding
            .unwrap_or_else(|| vec![0i8; EMBEDDING_DIM]);

        let mut manifest = AgentManifest {
            agent_id,
            public_key: public_key_bytes,
            capability_embedding: embedding,
            capability_tags: self.capability_tags,
            latency_sla_us: self.latency_sla_us,
            base_price_per_cu: self.base_price_per_cu,
            staked_amount: self.staked_amount,
            zk_capability_proof: self.zk_capability_proof,
            timestamp_ns,
            signature: vec![0u8; 64],
        };

        manifest.validate_structure()?;

        let hash = manifest.content_hash();
        let sig: ed25519_dalek::Signature = signing_key.sign(&hash);
        manifest.signature = sig.to_bytes().to_vec();

        Ok(manifest)
    }
}
