//! Typed error hierarchy for the AXON Protocol.

use thiserror::Error;

/// Top-level error type for all AXON operations.
#[derive(Debug, Error)]
pub enum AxonError {
    // ── Cryptographic errors ──────────────────────────────────────────────────
    #[error("Invalid Ed25519 public key encoding")]
    InvalidPublicKey,

    #[error("Invalid Ed25519 signature encoding")]
    InvalidSignature,

    #[error("Signature verification failed (tampered message or wrong key)")]
    SignatureVerificationFailed,

    // ── Manifest errors ───────────────────────────────────────────────────────
    #[error("Embedding has {actual} dimensions, expected {expected}")]
    EmbeddingDimensionMismatch { expected: usize, actual: usize },

    #[error("Manifest has too many capability tags: {count} (max {max})")]
    TooManyCapabilityTags { count: usize, max: usize },

    #[error("Capability tag exceeds maximum length: {len} bytes (max {max})")]
    CapabilityTagTooLong { len: usize, max: usize },

    // ── Handshake errors ──────────────────────────────────────────────────────
    #[error("Request rejected: {0:?}")]
    Rejected(RejectionReason),

    #[error(
        "Request targets agent {expected_hex} but this node is {local_hex}"
    )]
    TargetMismatch { expected_hex: String, local_hex: String },

    #[error(
        "Deadline {deadline_ns}ns is more than {max_offset_ns}ns in the future"
    )]
    DeadlineTooFar { deadline_ns: u128, max_offset_ns: u128 },

    #[error("Arithmetic overflow: max_compute_units × max_price_per_cu exceeds u64")]
    ArithmeticOverflow,

    // ── System errors ─────────────────────────────────────────────────────────
    #[error("System clock unavailable or regressed")]
    ClockError,

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Storage error: {0}")]
    Storage(String),
}

/// Structured reason for handshake rejection — returned to initiating agent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum RejectionReason {
    /// Agent is at capacity. `retry_after_ns` is a Unix-ns suggestion.
    CapacityExhausted { retry_after_ns: u128 },

    /// Initiator's max_price_per_cu < target's base_price_per_cu.
    PriceTooLow { min_acceptable_pico_sui: u64 },

    /// Cosine similarity between embeddings is below CAPABILITY_MATCH_THRESHOLD.
    CapabilityMismatch { actual_similarity: f32 },

    /// Escrow amount < max_compute_units × max_price_per_cu.
    InsufficientEscrow { required: u64, provided: u64 },

    /// Request deadline has passed or is already expired.
    DeadlineExpired,

    /// Initiator's cryptographic proof (signature or ZK) failed verification.
    CryptographicVerificationFailed,

    /// On-chain escrow object could not be verified (not found, wrong amount, etc.).
    EscrowVerificationFailed { detail: String },
}
