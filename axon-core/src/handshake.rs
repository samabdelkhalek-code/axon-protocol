//! Handshake Engine — deterministic request validation and commitment generation.
//!
//! This module runs on every agent node in the hot path.
//! It is pure (no I/O, no network, no allocation in the common case).
//! The caller (axon-daemon) resolves manifests from the DHT cache and
//! verifies the on-chain escrow asynchronously before invoking this module.

use blake3::Hasher;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::constants::{
    CAPABILITY_MATCH_THRESHOLD, CAPACITY_REJECT_THRESHOLD, CAPACITY_RETRY_DELAY_NS,
    COMMITMENT_DOMAIN_SEP, EMBEDDING_DIM, MAX_DEADLINE_OFFSET_NS,
};
use crate::errors::{AxonError, RejectionReason};
use crate::manifest::AgentManifest;
use crate::similarity::cosine_similarity_i8;

// ── Wire types ─────────────────────────────────────────────────────────────────

/// Initiating agent's request to engage a target agent for task execution.
///
/// The on-chain escrow (`escrow_object_id`) MUST be confirmed as existing and
/// funded by the routing layer before this reaches the target's `HandshakeEngine`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeRequest {
    /// Session nonce (UUIDv4, 16 random bytes). Prevents replay attacks.
    pub session_id: [u8; 16],

    /// Initiating agent's AXON ID.
    pub initiator_id: [u8; 32],

    /// Target agent's AXON ID (obtained from the DHT ANN matcher result).
    pub target_id: [u8; 32],

    /// Capability requirement embedding (768-dim, int8).
    /// Target re-verifies the cosine similarity against its own manifest.
    pub required_capability_embedding: Vec<i8>,

    /// BLAKE3 hash of the encrypted task payload.
    /// The payload itself is delivered via a separate Noise-protocol channel.
    /// This hash binds the handshake to a specific task, preventing substitution.
    pub task_payload_hash: [u8; 32],

    /// Upper bound on the number of compute units this request may consume.
    pub max_compute_units: u64,

    /// Maximum acceptable price per CU (picoSUI).
    pub max_price_per_cu: u64,

    /// Total escrow locked on-chain = `max_compute_units × max_price_per_cu`.
    /// Must match the on-chain escrow object value exactly.
    pub escrow_amount: u64,

    /// SUI object ID of the on-chain `AgentEscrow` object (32-byte SUI object ID).
    pub escrow_object_id: [u8; 32],

    /// Request expiry (Unix nanoseconds). The request is invalid after this point.
    pub deadline_ns: u128,

    /// Ed25519 signature by the initiator over `self.content_hash()`.
    pub initiator_signature: Vec<u8>,
}

impl HandshakeRequest {
    /// Canonical BLAKE3 hash covering all fields except `initiator_signature`.
    pub fn content_hash(&self) -> [u8; 32] {
        let emb_bytes: Vec<u8> = self.required_capability_embedding
            .iter()
            .map(|&x| x as u8)
            .collect();

        *Hasher::new()
            .update(&self.session_id)
            .update(&self.initiator_id)
            .update(&self.target_id)
            .update(&emb_bytes)
            .update(&self.task_payload_hash)
            .update(&self.max_compute_units.to_le_bytes())
            .update(&self.max_price_per_cu.to_le_bytes())
            .update(&self.escrow_amount.to_le_bytes())
            .update(&self.escrow_object_id)
            .update(&self.deadline_ns.to_le_bytes())
            .finalize()
            .as_bytes()
    }
}

/// Target agent's response to a `HandshakeRequest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    /// Mirrors `HandshakeRequest::session_id` for correlation.
    pub session_id: [u8; 16],

    /// Responding agent's AXON ID.
    pub responder_id: [u8; 32],

    /// Outcome of capability and price negotiation.
    pub status: HandshakeStatus,

    /// Agreed price per CU (≤ `max_price_per_cu` from the request). Zero if rejected.
    pub agreed_price_per_cu: u64,

    /// Settlement trigger hash.
    /// `BLAKE3(COMMITMENT_DOMAIN_SEP || preimage)`, where the preimage is:
    /// `BLAKE3(DOMAIN_SEP || session_id || task_payload_hash || responder_id || price_bytes)`
    ///
    /// The responder stores the preimage internally and submits it on-chain
    /// after task completion to release the escrow.
    pub commitment_hash: [u8; 32],

    /// Unix nanosecond timestamp of this response.
    pub timestamp_ns: u128,

    /// Ed25519 signature by the responder over `BLAKE3(all fields above except this)`.
    pub responder_signature: Vec<u8>,
}

/// Outcome of the handshake negotiation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HandshakeStatus {
    /// Task accepted at `agreed_price_per_cu`. Execution commences immediately.
    Accepted,

    /// Target cannot accept this request.
    Rejected { reason: RejectionReason },

    /// Target can accept but requires a higher price.
    /// Initiator may re-send with updated `max_price_per_cu`.
    CounterOffer { proposed_price_per_cu: u64 },
}

// ── Engine ─────────────────────────────────────────────────────────────────────

/// Deterministic handshake validation and commitment generation engine.
///
/// No I/O, no heap allocation in the hot path, no global state.
/// Instantiate once per agent node; share across async tasks via `Arc`.
pub struct HandshakeEngine {
    /// This node's AXON ID (used to reject misdirected requests early).
    pub local_agent_id: [u8; 32],

    /// This node's signed manifest (used for capability comparison and pricing).
    pub local_manifest: AgentManifest,
}

impl HandshakeEngine {
    /// Validate an inbound `HandshakeRequest`.
    ///
    /// # Arguments
    /// - `request`: The inbound request to validate.
    /// - `initiator_manifest`: Fetched from the DHT cache by the daemon layer.
    ///   The daemon must call `manifest.verify_signature()` before passing it here.
    /// - `escrow_verified`: Result of the async on-chain escrow object lookup.
    ///   `true` iff the object exists, is owned by this escrow contract, and holds
    ///   at least `request.escrow_amount` picoSUI.
    /// - `current_capacity`: Fraction of this node's concurrency slots currently
    ///   in use, in `[0.0, 1.0]`. Pass `0.0` if capacity tracking is not yet
    ///   implemented.
    ///
    /// # Returns
    /// `Ok(ValidationResult)` → build a `HandshakeResponse` with status `Accepted`.
    /// `Err(AxonError::Rejected(_))` → build a `HandshakeResponse` with status `Rejected`.
    /// `Err(other)` → internal error; do not respond to the initiator.
    pub fn validate_request(
        &self,
        request: &HandshakeRequest,
        initiator_manifest: &AgentManifest,
        escrow_verified: bool,
        current_capacity: f32,
    ) -> Result<ValidationResult, AxonError> {
        use ed25519_dalek::Verifier;

        let now_ns = Self::now_ns()?;

        // ── 1. Request is not expired ─────────────────────────────────────────
        if now_ns > request.deadline_ns {
            return Err(AxonError::Rejected(RejectionReason::DeadlineExpired));
        }
        // Deadline must not be unreasonably far (resource-lock attack mitigation).
        if request.deadline_ns > now_ns + MAX_DEADLINE_OFFSET_NS {
            return Err(AxonError::DeadlineTooFar {
                deadline_ns: request.deadline_ns,
                max_offset_ns: MAX_DEADLINE_OFFSET_NS,
            });
        }

        // ── 2. Request is addressed to this node ──────────────────────────────
        if request.target_id != self.local_agent_id {
            return Err(AxonError::TargetMismatch {
                expected_hex: hex::encode(request.target_id),
                local_hex: hex::encode(self.local_agent_id),
            });
        }

        // ── 3. Initiator signature is valid ───────────────────────────────────
        let vk = VerifyingKey::from_bytes(&initiator_manifest.public_key)
            .map_err(|_| AxonError::InvalidPublicKey)?;
        let sig = Signature::from_slice(&request.initiator_signature)
            .map_err(|_| AxonError::InvalidSignature)?;
        vk.verify_strict(&request.content_hash(), &sig)
            .map_err(|_| AxonError::SignatureVerificationFailed)?;

        // ── 4. Escrow covers worst-case cost ──────────────────────────────────
        let max_total_cost = request
            .max_compute_units
            .checked_mul(request.max_price_per_cu)
            .ok_or(AxonError::ArithmeticOverflow)?;

        if request.escrow_amount < max_total_cost {
            return Err(AxonError::Rejected(RejectionReason::InsufficientEscrow {
                required: max_total_cost,
                provided: request.escrow_amount,
            }));
        }

        // ── 5. On-chain escrow is confirmed ───────────────────────────────────
        if !escrow_verified {
            return Err(AxonError::Rejected(RejectionReason::EscrowVerificationFailed {
                detail: "escrow object not found or underfunded".into(),
            }));
        }

        // ── 6. Capability embedding matches this agent ─────────────────────────
        if request.required_capability_embedding.len() != EMBEDDING_DIM {
            return Err(AxonError::EmbeddingDimensionMismatch {
                expected: EMBEDDING_DIM,
                actual: request.required_capability_embedding.len(),
            });
        }
        let similarity = cosine_similarity_i8(
            &request.required_capability_embedding,
            &self.local_manifest.capability_embedding,
        );
        if similarity < CAPABILITY_MATCH_THRESHOLD {
            return Err(AxonError::Rejected(RejectionReason::CapabilityMismatch {
                actual_similarity: similarity,
            }));
        }

        // ── 7. Node has capacity ───────────────────────────────────────────────
        if current_capacity >= CAPACITY_REJECT_THRESHOLD {
            return Err(AxonError::Rejected(RejectionReason::CapacityExhausted {
                retry_after_ns: now_ns + CAPACITY_RETRY_DELAY_NS,
            }));
        }

        // ── 8. Initiator's offered price meets our floor ──────────────────────
        let floor = self.local_manifest.base_price_per_cu;
        if request.max_price_per_cu < floor {
            return Err(AxonError::Rejected(RejectionReason::PriceTooLow {
                min_acceptable_pico_sui: floor,
            }));
        }

        Ok(ValidationResult {
            session_id:             request.session_id,
            capability_match_score: similarity,
            // Accept at our floor price; the initiator may have budgeted higher.
            // The excess stays in escrow and is refunded on settlement.
            agreed_price_per_cu:    floor,
            max_compute_units:      request.max_compute_units,
        })
    }

    /// Generate the commitment preimage and hash for on-chain settlement.
    ///
    /// The **preimage** is stored securely by the responder and submitted to the
    /// settlement contract after the task completes. The **commitment_hash** is
    /// published in the `HandshakeResponse` and stored in the on-chain escrow.
    ///
    /// This construction ensures:
    /// - Only the responder (who knows the preimage) can settle the escrow.
    /// - The preimage commits to the exact session, task, and price —
    ///   it cannot be reused for a different session or price.
    ///
    /// # Returns
    /// `(preimage: [u8; 32], commitment_hash: [u8; 32])`
    pub fn generate_commitment(
        session_id:          &[u8; 16],
        task_payload_hash:   &[u8; 32],
        responder_id:        &[u8; 32],
        agreed_price_per_cu: u64,
    ) -> ([u8; 32], [u8; 32]) {
        // The preimage uniquely encodes this session at the agreed price.
        let preimage = *Hasher::new()
            .update(COMMITMENT_DOMAIN_SEP)
            .update(session_id)
            .update(task_payload_hash)
            .update(responder_id)
            .update(&agreed_price_per_cu.to_le_bytes())
            .finalize()
            .as_bytes();

        // Commitment = BLAKE3(preimage). The settlement contract verifies this.
        let commitment_hash = *Hasher::new()
            .update(&preimage)
            .finalize()
            .as_bytes();

        (preimage, commitment_hash)
    }

    fn now_ns() -> Result<u128, AxonError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .map_err(|_| AxonError::ClockError)
    }
}

/// Successful validation result — passed to response builder in the daemon.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub session_id:             [u8; 16],
    pub capability_match_score: f32,
    pub agreed_price_per_cu:    u64,
    pub max_compute_units:      u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::EMBEDDING_DIM;

    fn make_embedding(val: i8) -> Vec<i8> { vec![val; EMBEDDING_DIM] }

    fn gen_key() -> ed25519_dalek::SigningKey {
        use rand::rngs::OsRng;
        ed25519_dalek::SigningKey::generate(&mut OsRng)
    }

    fn make_manifest(key: &ed25519_dalek::SigningKey, price: u64, emb_val: i8) -> AgentManifest {
        crate::manifest::ManifestBuilder::new()
            .embedding(make_embedding(emb_val))
            .tags(vec!["test".into()])
            .latency_sla_us(5_000)
            .price_per_cu(price)
            .stake(100_000_000)
            .build(key, &[0u8; 32])
            .expect("build manifest")
    }

    #[test]
    fn generate_commitment_is_deterministic() {
        let (p1, h1) = HandshakeEngine::generate_commitment(
            &[1u8; 16], &[2u8; 32], &[3u8; 32], 1000,
        );
        let (p2, h2) = HandshakeEngine::generate_commitment(
            &[1u8; 16], &[2u8; 32], &[3u8; 32], 1000,
        );
        assert_eq!(p1, p2);
        assert_eq!(h1, h2);
    }

    #[test]
    fn commitment_changes_with_price() {
        let (p1, _) = HandshakeEngine::generate_commitment(
            &[1u8; 16], &[2u8; 32], &[3u8; 32], 1000,
        );
        let (p2, _) = HandshakeEngine::generate_commitment(
            &[1u8; 16], &[2u8; 32], &[3u8; 32], 2000,
        );
        assert_ne!(p1, p2, "Different prices must produce different preimages");
    }

    #[test]
    fn preimage_hashes_to_commitment() {
        let (preimage, commitment) = HandshakeEngine::generate_commitment(
            &[1u8; 16], &[2u8; 32], &[3u8; 32], 500,
        );
        let computed = *Hasher::new().update(&preimage).finalize().as_bytes();
        assert_eq!(computed, commitment);
    }

    #[test]
    fn validate_request_accepts_valid_request() {
        let initiator_key = gen_key();
        let responder_key = gen_key();

        let responder_manifest = make_manifest(&responder_key, 100, 100);
        let initiator_manifest = make_manifest(&initiator_key, 100, 100);

        let engine = HandshakeEngine {
            local_agent_id: responder_manifest.agent_id,
            local_manifest: responder_manifest,
        };

        // Build a valid request
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let mut req = HandshakeRequest {
            session_id: rand::random(),
            initiator_id: initiator_manifest.agent_id,
            target_id: engine.local_agent_id,
            required_capability_embedding: make_embedding(100),
            task_payload_hash: [1u8; 32],
            max_compute_units: 100,
            max_price_per_cu: 200, // higher than floor
            escrow_amount: 20_000, // = 100 * 200
            escrow_object_id: [0u8; 32],
            deadline_ns: now_ns + 60_000_000_000, // 60 seconds from now
            initiator_signature: [0u8; 64],
        };

        // Sign the request
        use ed25519_dalek::Signer;
        let sig = initiator_key.sign(&req.content_hash());
        req.initiator_signature = sig.to_bytes();

        let result = engine.validate_request(&req, &initiator_manifest, true, 0.0);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

        let vr = result.unwrap();
        assert_eq!(vr.agreed_price_per_cu, 100); // our floor, not initiator's max
    }

    #[test]
    fn validate_request_rejects_expired_deadline() {
        let responder_key = gen_key();
        let initiator_key = gen_key();
        let responder_manifest = make_manifest(&responder_key, 100, 100);
        let initiator_manifest = make_manifest(&initiator_key, 100, 100);

        let engine = HandshakeEngine {
            local_agent_id: responder_manifest.agent_id,
            local_manifest: responder_manifest,
        };

        let mut req = HandshakeRequest {
            session_id: rand::random(),
            initiator_id: initiator_manifest.agent_id,
            target_id: engine.local_agent_id,
            required_capability_embedding: make_embedding(100),
            task_payload_hash: [1u8; 32],
            max_compute_units: 100,
            max_price_per_cu: 200,
            escrow_amount: 20_000,
            escrow_object_id: [0u8; 32],
            deadline_ns: 1, // expired: Unix epoch + 1 nanosecond
            initiator_signature: [0u8; 64],
        };

        use ed25519_dalek::Signer;
        req.initiator_signature = initiator_key.sign(&req.content_hash()).to_bytes();

        let err = engine.validate_request(&req, &initiator_manifest, true, 0.0)
            .unwrap_err();

        assert!(matches!(
            err,
            AxonError::Rejected(RejectionReason::DeadlineExpired)
        ));
    }
}
