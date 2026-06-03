use libp2p::{
    kad::{self, store::MemoryStore, QueryId},
    swarm::{NetworkBehaviour, SwarmEvent, Swarm},
    identity, PeerId, Multiaddr,
};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::dht::DhtCommand;
use std::collections::HashMap;
use tokio::sync::oneshot;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "AxonBehaviourEvent")]
pub struct AxonBehaviour {
    pub kademlia: kad::Behaviour<MemoryStore>,
}

#[derive(Debug)]
pub enum AxonBehaviourEvent {
    Kademlia(kad::Event),
}

impl From<kad::Event> for AxonBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        AxonBehaviourEvent::Kademlia(event)
    }
}

pub async fn run_swarm(
    mut swarm: Swarm<AxonBehaviour>,
    mut command_rx: mpsc::Receiver<DhtCommand>,
) -> anyhow::Result<()> {
    let mut pending_queries = HashMap::<QueryId, oneshot::Sender<Result<(), String>>>::new();

    loop {
        tokio::select! {
            command = command_rx.recv() => {
                match command {
                    Some(DhtCommand::Publish { manifest, reply }) => {
                        let key = libp2p::kad::RecordKey::new(&manifest.agent_id);
                        let value = serde_json::to_vec(&manifest).unwrap();
                        let record = libp2p::kad::Record {
                            key,
                            value,
                            publisher: None,
                            expires: None,
                        };
                        match swarm.behaviour_mut().kademlia.put_record(record, libp2p::kad::Quorum::One) {
                            Ok(query_id) => {
                                pending_queries.insert(query_id, reply);
                            }
                            Err(e) => {
                                let _ = reply.send(Err(e.to_string()));
                            }
                        }
                    }
                    Some(DhtCommand::Get { agent_id, reply }) => {
                        // TODO: Implement get_record logic
                        let _ = reply.send(None);
                    }
                    None => break,
                }
            }
            event = swarm.select_next_some() => {
                if let SwarmEvent::Behaviour(AxonBehaviourEvent::Kademlia(event)) = event {
                    match event {
                        kad::Event::OutboundQueryProgressed { id, result, .. } => {
                            if let Some(reply) = pending_queries.remove(&id) {
                                match result {
                                    libp2p::kad::QueryResult::PutRecord(Ok(_)) => {
                                        let _ = reply.send(Ok(()));
                                    }
                                    libp2p::kad::QueryResult::PutRecord(Err(e)) => {
                                        let _ = reply.send(Err(e.to_string()));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn setup_swarm(
    key_bytes: &[u8; 32],
    listen_addr: &str,
) -> anyhow::Result<Swarm<AxonBehaviour>> {
    let id_keys = identity::Keypair::ed25519_from_bytes(key_bytes.to_owned())?;
    let local_peer_id = PeerId::from(id_keys.public());
    
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let mut kad_config = kad::Config::default();
            kad_config.set_record_ttl(Some(Duration::from_secs(24 * 3600)));
            let store = MemoryStore::new(key.public().to_peer_id());
            AxonBehaviour {
                kademlia: kad::Behaviour::with_config(key.public().to_peer_id(), store, kad_config),
            }
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let addr: Multiaddr = listen_addr.parse()?;
    swarm.listen_on(addr)?;

    Ok(swarm)
}
