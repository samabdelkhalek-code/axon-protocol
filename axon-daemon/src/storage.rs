use sled::Db;
use std::path::Path;
use anyhow::Result;

/// Persistent storage for session preimages and metadata.
pub struct Storage {
    db: Db,
}

impl Storage {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    /// Store a preimage for a session.
    pub fn store_preimage(&self, session_id: &[u8; 16], preimage: &[u8; 32]) -> Result<()> {
        self.db.insert(session_id, preimage)?;
        self.db.flush()?;
        Ok(())
    }

    /// Retrieve a preimage by session ID.
    pub fn get_preimage(&self, session_id: &[u8; 16]) -> Result<Option<[u8; 32]>> {
        let val = self.db.get(session_id)?;
        match val {
            Some(ivec) => {
                let bytes: [u8; 32] = ivec.as_ref().try_into()
                    .map_err(|_| anyhow::anyhow!("Invalid preimage length in DB"))?;
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }

    /// Remove a preimage after successful settlement.
    pub fn remove_preimage(&self, session_id: &[u8; 16]) -> Result<()> {
        self.db.remove(session_id)?;
        Ok(())
    }
}
