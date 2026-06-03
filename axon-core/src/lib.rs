//! # axon-core
//!
//! Core protocol types, cryptographic primitives, and validation logic for
//! the AXON Agent-to-Agent mesh routing protocol.
//!
//! ## Module layout
//!
//! | Module | Purpose |
//! |--------|---------|
//! | `constants` | Protocol parameters (single source of truth) |
//! | `errors`    | Typed error hierarchy |
//! | `manifest`  | `AgentManifest` — signed capability declaration |
//! | `handshake` | Handshake wire types and validation engine |
//! | `similarity`| Cosine similarity over int8 embeddings |
//! | `settlement`| Off-chain session tracking and price oracle |

pub mod constants;
pub mod errors;
pub mod handshake;
pub mod manifest;
pub mod settlement;
pub mod similarity;

// Re-export the most commonly used types at the crate root.
pub use constants::*;
pub use errors::{AxonError, RejectionReason};
pub use handshake::{
    HandshakeEngine, HandshakeRequest, HandshakeResponse, HandshakeStatus, ValidationResult,
};
pub use manifest::{AgentManifest, ManifestBuilder};
pub use settlement::{PriceOracle, SessionRecord, SessionStatus};
pub use similarity::{composite_rank_score, cosine_similarity_i8};
