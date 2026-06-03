//! Off-chain settlement types — mirror of the on-chain Move contract types.
//!
//! These types are used by axon-daemon to track in-flight sessions and
//! submit settlement transactions to the SUI network.

use serde::{Deserialize, Serialize};

/// Status of a session from the daemon's perspective.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    /// HandshakeRequest sent, waiting for response.
    PendingResponse,

    /// HandshakeResponse received, task in execution.
    Executing,

    /// Task complete; settlement transaction submitted on-chain.
    SettlementSubmitted { tx_digest: String },

    /// Settlement confirmed on-chain with finality.
    Settled {
        tx_digest: String,
        responder_payment_pico_sui: u64,
        protocol_fee_pico_sui: u64,
        refund_pico_sui: u64,
    },

    /// Deadline passed without settlement; reclaim submitted.
    Reclaimed { tx_digest: String },

    /// Session failed with an error.
    Failed { reason: String },
}

/// In-memory record of an active or recently completed session.
/// Persisted to sled by axon-daemon for crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    /// 16-byte session UUID.
    pub session_id: [u8; 16],

    /// The other party's AXON ID (responder from initiator's view, or vice versa).
    pub counterparty_id: [u8; 32],

    /// On-chain escrow SUI object ID.
    pub escrow_object_id: [u8; 32],

    /// Commitment hash (published in HandshakeResponse).
    pub commitment_hash: [u8; 32],

    /// Preimage (only set on the responder side after task completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage: Option<[u8; 32]>,

    /// Agreed price per CU (picoSUI).
    pub agreed_price_per_cu: u64,

    /// Actual CUs consumed (set by responder on completion).
    pub actual_compute_units: u64,

    /// Request deadline (Unix nanoseconds).
    pub deadline_ns: u128,

    /// Current session status.
    pub status: SessionStatus,

    /// Unix nanosecond timestamp of the last status update.
    pub updated_at_ns: u128,
}

impl SessionRecord {
    pub fn session_id_hex(&self) -> String {
        hex::encode(self.session_id)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            SessionStatus::Settled { .. }
                | SessionStatus::Reclaimed { .. }
                | SessionStatus::Failed { .. }
        )
    }
}

/// Price oracle state maintained by the daemon.
/// Tracks the exponential moving average of clearing prices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceOracle {
    /// EMA of clearing prices in picoSUI per CU.
    pub ema_price_per_cu: u64,

    /// Number of settlements included in the EMA.
    pub sample_count: u64,
}

impl PriceOracle {
    pub fn new(initial_price: u64) -> Self {
        Self { ema_price_per_cu: initial_price, sample_count: 0 }
    }

    /// Update the EMA with a new clearing price.
    /// α = 0.05: each new observation contributes 5% of the EMA weight.
    pub fn update(&mut self, clearing_price: u64) {
        // Integer EMA: new = (19 * old + clearing_price) / 20
        // Equivalent to: new = (1 - 0.05) * old + 0.05 * sample
        self.ema_price_per_cu = (19 * self.ema_price_per_cu + clearing_price) / 20;
        self.sample_count += 1;
    }

    /// Returns true if a price is above the circuit-breaker threshold (10× EMA).
    pub fn is_above_circuit_breaker(&self, price: u64) -> bool {
        price > self.ema_price_per_cu.saturating_mul(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_oracle_converges() {
        let mut oracle = PriceOracle::new(1_000);
        // Feed 100 samples at 2_000; EMA should approach 2_000
        for _ in 0..100 {
            oracle.update(2_000);
        }
        // After 100 iterations with α=0.05, should be > 1900
        assert!(oracle.ema_price_per_cu > 1_900, "EMA: {}", oracle.ema_price_per_cu);
    }

    #[test]
    fn circuit_breaker_triggers_at_10x() {
        let oracle = PriceOracle::new(1_000);
        assert!(oracle.is_above_circuit_breaker(10_001));
        assert!(!oracle.is_above_circuit_breaker(10_000));
    }
}
