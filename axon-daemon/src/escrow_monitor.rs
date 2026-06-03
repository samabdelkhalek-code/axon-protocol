use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashSet;

/// Monitors the SUI blockchain for AXON escrow events (Mock implementation).
pub struct EscrowMonitor {
    rpc_url: String,
    package_id: String,
    /// Set of verified escrow object IDs.
    verified_escrows: Arc<RwLock<HashSet<[u8; 32]>>>,
}

impl EscrowMonitor {
    pub fn new(rpc_url: String, package_id: String) -> Self {
        Self {
            rpc_url,
            package_id,
            verified_escrows: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Returns true if the escrow object has been seen and verified on-chain.
    pub async fn is_verified(&self, escrow_id: &[u8; 32]) -> bool {
        // MOCK: In dev mode, we automatically verify any escrow ID starting with 0x0
        if escrow_id.iter().all(|&b| b == 0) {
            return true;
        }
        self.verified_escrows.read().await.contains(escrow_id)
    }

    /// Background loop to poll for SUI events.
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        tracing::info!(rpc = %self.rpc_url, package = %self.package_id, "Starting MOCK SUI escrow monitor");
        
        // In dev mode, we just keep the loop alive.
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            tracing::debug!("Mock monitor polling...");
        }
    }
}
