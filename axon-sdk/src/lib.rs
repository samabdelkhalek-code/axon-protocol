//! # axon-sdk
//!
//! One-call developer SDK for registering agents in the AXON protocol.
//!
//! ## Quickstart
//! ```rust,ignore
//! use axon_sdk::register;
//!
//! let result = register(
//!     "Summarises long documents into structured bullet points",
//!     1_000,          // 1000 picoSUI per compute unit
//!     50_000_000,     // 50M picoSUI stake
//!     &AXON_DEVNET_GENESIS,
//! ).await?;
//!
//! println!("Registered as {}", hex::encode(result.agent_id));
//! ```

pub mod register;
pub mod discovery;

pub use register::{register, RegisterResult};
pub use discovery::{discover, DiscoverResult};

/// Genesis block hash for AXON devnet.
/// Replace with testnet / mainnet values when available.
pub const AXON_DEVNET_GENESIS: [u8; 32] = [
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,
];
