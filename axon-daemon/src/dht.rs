//! Kademlia DHT interface — trait and in-process stub implementation.
//!
//! The production implementation wraps libp2p-kad. The stub allows the rest
//! of the daemon to compile and be tested without a live P2P network.

use axon_core::AgentManifest;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Core DHT operations required by the routing layer.
pub trait Dht: Send + Sync {
    /// Publish a signed manifest to the DHT.
    fn publish(&self, manifest: &AgentManifest) -> Result<(), String>;

    /// Retrieve a manifest by agent ID. Returns `None` if not found locally.
    fn get(&self, agent_id: &[u8; 32]) -> Option<AgentManifest>;

    /// Remove a manifest from the local store (e.g., on TTL expiry).
    fn remove(&self, agent_id: &[u8; 32]);

    /// Number of manifests currently stored locally.
    fn local_count(&self) -> usize;
}

/// In-process DHT stub — correctness without network I/O.
///
/// Replace with `Libp2pDht` (wraps `libp2p::kad::Kademlia`) for production.
pub struct InProcessDht {
    store: Arc<RwLock<HashMap<[u8; 32], AgentManifest>>>,
}

impl InProcessDht {
    pub fn new() -> Self {
        Self { store: Arc::new(RwLock::new(HashMap::new())) }
    }
}

impl Default for InProcessDht { fn default() -> Self { Self::new() } }

impl Dht for InProcessDht {
    fn publish(&self, manifest: &AgentManifest) -> Result<(), String> {
        // Verify signature before storing.
        manifest.verify_signature()
            .map_err(|e| format!("Invalid manifest signature: {e}"))?;

        let mut store = self.store.write().unwrap();
        let id = manifest.agent_id;

        // Only update if newer.
        if let Some(existing) = store.get(&id) {
            if existing.timestamp_ns >= manifest.timestamp_ns {
                return Ok(()); // not an error — just a no-op
            }
        }

        store.insert(id, manifest.clone());
        tracing::debug!(
            agent_id = manifest.short_id(),
            "Published manifest to local DHT store"
        );
        Ok(())
    }

    fn get(&self, agent_id: &[u8; 32]) -> Option<AgentManifest> {
        self.store.read().unwrap().get(agent_id).cloned()
    }

    fn remove(&self, agent_id: &[u8; 32]) {
        self.store.write().unwrap().remove(agent_id);
    }

    fn local_count(&self) -> usize {
        self.store.read().unwrap().len()
    }
}

use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// Commands sent to the background swarm task.
pub enum DhtCommand {
    Publish {
        manifest: AgentManifest,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Get {
        agent_id: [u8; 32],
        reply: oneshot::Sender<Option<AgentManifest>>,
    },
}

pub struct Libp2pDht {
    tx: mpsc::Sender<DhtCommand>,
    local_store: Arc<RwLock<HashMap<[u8; 32], AgentManifest>>>,
}

impl Libp2pDht {
    pub fn new(tx: mpsc::Sender<DhtCommand>, local_store: Arc<RwLock<HashMap<[u8; 32], AgentManifest>>>) -> Self {
        Self { tx, local_store }
    }
}

impl Dht for Libp2pDht {
    fn publish(&self, manifest: &AgentManifest) -> Result<(), String> {
        // Still verify locally
        manifest.verify_signature()
            .map_err(|e| format!("Invalid manifest signature: {e}"))?;

        // Always keep a local copy so get() works even without remote peers.
        self.local_store.write().unwrap().insert(manifest.agent_id, manifest.clone());

        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.try_send(DhtCommand::Publish {
            manifest: manifest.clone(),
            reply: reply_tx,
        });

        // For now, blocking wait to satisfy the sync trait (can be made async later)
        futures::executor::block_on(async {
            match reply_rx.await {
                Ok(res) => res,
                Err(_) => Err("Swarm task dropped".into()),
            }
        })
    }

    fn get(&self, agent_id: &[u8; 32]) -> Option<AgentManifest> {
        // Check local store first
        if let Some(m) = self.local_store.read().unwrap().get(agent_id) {
            return Some(m.clone());
        }
        
        // Then query the DHT (blocking wait again)
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.try_send(DhtCommand::Get {
            agent_id: *agent_id,
            reply: reply_tx,
        });

        futures::executor::block_on(async {
            reply_rx.await.ok().flatten()
        })
    }

    fn remove(&self, agent_id: &[u8; 32]) {
        self.local_store.write().unwrap().remove(agent_id);
    }

    fn local_count(&self) -> usize {
        self.local_store.read().unwrap().len()
    }
}
