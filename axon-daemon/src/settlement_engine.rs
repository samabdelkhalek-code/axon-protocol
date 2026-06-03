use crate::storage::Storage;
use std::sync::Arc;

/// Responsible for submitting settlement transactions to the SUI blockchain.
pub struct SettlementEngine {
    rpc_url: String,
    storage: Arc<Storage>,
}

impl SettlementEngine {
    pub fn new(rpc_url: String, storage: Arc<Storage>) -> Self {
        Self { rpc_url, storage }
    }

    /// Submit a settlement transaction for a completed task.
    ///
    /// # Arguments
    /// - `session_id`: The ID of the session to settle.
    /// - `actual_compute_units`: The actual amount of work performed.
    /// - `agreed_price`: The price per CU agreed during handshake.
    pub async fn settle_session(
        &self,
        session_id: [u8; 16],
        actual_compute_units: u64,
        agreed_price: u64,
    ) -> anyhow::Result<()> {
        let preimage = self.storage.get_preimage(&session_id)?
            .ok_or_else(|| anyhow::anyhow!("Preimage not found for session"))?;

        tracing::info!(
            session_id = hex::encode(session_id),
            cu = actual_compute_units,
            "Submitting settlement to SUI"
        );

        // MVP: This is where we would call the SUI smart contract.
        // In production:
        // let sui_client = SuiClientBuilder::default().build(&self.rpc_url).await?;
        // let tx = build_settle_transaction(preimage, actual_compute_units, agreed_price);
        // sui_client.transaction_block_builder().execute(tx).await?;

        // After successful on-chain settlement, clear the preimage
        self.storage.remove_preimage(&session_id)?;
        
        tracing::info!(session_id = hex::encode(session_id), "Settlement successful");
        Ok(())
    }
}
