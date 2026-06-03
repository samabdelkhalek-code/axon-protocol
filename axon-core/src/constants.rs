//! AXON Protocol constants — single source of truth for all protocol parameters.
//!
//! Any change here is a protocol version bump.

/// Minimum cosine similarity between requirement and capability embeddings.
/// Below 0.82, the routing layer should not have forwarded the request.
pub const CAPABILITY_MATCH_THRESHOLD: f32 = 0.82;

/// Embedding dimensionality — matches sentence-transformers/all-mpnet-base-v2.
/// Pinned at genesis; changing this requires a protocol migration.
pub const EMBEDDING_DIM: usize = 768;

/// Protocol fee in basis points. 30 bps = 0.30% of every settled escrow.
pub const PROTOCOL_FEE_BPS: u64 = 30;

/// Maximum allowed deadline offset from now (nanoseconds = 1 hour).
/// Prevents long-duration resource-lock attacks on agent capacity.
pub const MAX_DEADLINE_OFFSET_NS: u128 = 3_600_000_000_000;

/// Domain separator for commitment hash generation.
/// Prevents cross-protocol or cross-version commitment collision.
pub const COMMITMENT_DOMAIN_SEP: &[u8] = b"AXON_COMMITMENT_V1:";

/// Initial EigenTrust score assigned to newly registered agents (out of 1_000_000).
pub const EIGENTRUST_INITIAL_SCORE: u64 = 1_000;

/// Maximum EigenTrust score. Values are clamped to this on every update.
pub const EIGENTRUST_MAX_SCORE: u64 = 1_000_000;

/// EMA decay numerator: score_new = score_old * NUM / DEN ± delta.
pub const EIGENTRUST_EMA_NUM: u64 = 9;
pub const EIGENTRUST_EMA_DEN: u64 = 10;

/// EigenTrust increment on successful settlement (0.1 × 1_000_000).
pub const EIGENTRUST_SUCCESS_INCREMENT: u64 = 100_000;

/// Minimum reputation score for a target to be considered in routing.
pub const MIN_REPUTATION_FOR_ROUTING: u64 = 100;

/// Capacity threshold above which new requests are rejected.
/// 0.95 = reject at 95% load, reserving headroom for in-flight sessions.
pub const CAPACITY_REJECT_THRESHOLD: f32 = 0.95;

/// Suggested retry delay (ns) returned when capacity is exhausted.
pub const CAPACITY_RETRY_DELAY_NS: u128 = 5_000_000_000; // 5 seconds
