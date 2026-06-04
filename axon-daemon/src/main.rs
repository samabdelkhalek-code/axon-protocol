//! axond — AXON Protocol Daemon
//!
//! Entry point for the AXON daemon node. Responsibilities:
//!   - Load or generate the Ed25519 signing key
//!   - Publish this node's AgentManifest to the DHT
//!   - Run the HNSW index over known manifests
//!   - Route inbound HandshakeRequests to the HandshakeEngine
//!   - Monitor on-chain escrow objects (SUI RPC)
//!   - Submit settlement and reclaim transactions

mod config;
mod dht;
mod hnsw_index;
mod grpc;
mod p2p;
mod escrow_monitor;
mod storage;
mod settlement_engine;

use axon_core::{AgentManifest, HandshakeEngine, ManifestBuilder, EMBEDDING_DIM};
use clap::Parser;
use config::Config;
use dht::{Dht, Libp2pDht, DhtCommand};
use ed25519_dalek::SigningKey;
use hnsw_index::HnswIndex;
use rand::rngs::OsRng;
use std::sync::{Arc, RwLock};
use tonic::transport::Server;
use grpc::{DiscoveryImpl, HandshakeImpl, SettlementImpl};
use grpc::axon_v1::discovery_service_server::DiscoveryServiceServer;
use grpc::axon_v1::handshake_service_server::HandshakeServiceServer;
use grpc::axon_v1::settlement_service_server::SettlementServiceServer;
use tokio::sync::mpsc;
use std::collections::HashMap;
use escrow_monitor::EscrowMonitor;
use storage::Storage;
use settlement_engine::SettlementEngine;

/// Global daemon state shared across async tasks.
pub struct DaemonState {
    config:   Config,
    key:      SigningKey,
    manifest: AgentManifest,
    dht:      Arc<dyn Dht>,
    index:    Arc<RwLock<HnswIndex>>,
    engine:   HandshakeEngine,
    monitor:  Arc<EscrowMonitor>,
    storage:  Arc<Storage>,
    settlement: Arc<SettlementEngine>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::parse();

    // Initialise structured logging
    tracing_subscriber::fmt()
        .with_env_filter(&cfg.log_level)
        .with_target(false)
        .json()
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "axond starting");

    // Load or generate the signing key
    let key = load_or_generate_key(&cfg.key_file)?;
    tracing::info!(
        public_key = hex::encode(key.verifying_key().as_bytes()),
        "Identity loaded"
    );

    // Build this node's manifest
    // TODO: call the embedding service to generate a real capability embedding
    //       from the agent's system prompt file.
    let placeholder_embedding: Vec<i8> = (0..EMBEDDING_DIM)
        .map(|i| ((i % 127) as i8).wrapping_sub(63))
        .collect();

    let manifest = ManifestBuilder::new()
        .embedding(placeholder_embedding.clone())
        .tags(vec!["axon-relay-node".into()])
        .latency_sla_us(5_000)
        .price_per_cu(1_000)
        .stake(10_000_000_000)
        .build(&key, &cfg.genesis_hash_bytes())?;

    tracing::info!(
        agent_id   = manifest.short_id(),
        price_per_cu = manifest.base_price_per_cu,
        "Manifest built"
    );

    // Initialise DHT and HNSW index
    let (dht_tx, dht_rx) = mpsc::channel::<DhtCommand>(100);
    let local_store = Arc::new(RwLock::new(HashMap::new()));
    
    // Start P2P swarm in background
    let swarm = p2p::setup_swarm(key.as_bytes(), &cfg.listen_addr).await?;
    tokio::spawn(p2p::run_swarm(swarm, dht_rx));

    let dht: Arc<dyn Dht> = Arc::new(Libp2pDht::new(dht_tx, local_store));
    dht.publish(&manifest).map_err(|e| anyhow::anyhow!(e))?;

    let mut index = HnswIndex::new();
    index.upsert(manifest.clone());
    let index = Arc::new(RwLock::new(index));

    // Build the handshake engine
    let engine = HandshakeEngine {
        local_agent_id: manifest.agent_id,
        local_manifest:  manifest.clone(),
    };

    let monitor = Arc::new(EscrowMonitor::new(
        cfg.sui_rpc.clone(),
        cfg.treasury_address.clone() // Assuming treasury_address is near package_id for now
    ));
    tokio::spawn(monitor.clone().run());

    let storage = Arc::new(Storage::new(&cfg.db_path)?);
    let settlement = Arc::new(SettlementEngine::new(cfg.sui_rpc.clone(), storage.clone()));

    let state = Arc::new(DaemonState {
        config: cfg.clone(),
        key,
        manifest,
        dht,
        index,
        engine,
        monitor,
        storage,
        settlement,
    });

    tracing::info!(
        listen = cfg.listen_addr,
        sui_rpc = cfg.sui_rpc,
        "Daemon initialised — starting gRPC server"
    );

    // TODO: Start libp2p swarm
    // TODO: Start SUI escrow monitor task
    // TODO: Start DHT bootstrap task

    let grpc_addr = std::env::var("AXON_GRPC_ADDR").unwrap_or_else(|_| "0.0.0.0:50051".to_string());
    let addr = grpc_addr.parse()?;
    let discovery = DiscoveryImpl::new(state.clone());
    let handshake = HandshakeImpl::new(state.clone());
    let settlement = SettlementImpl::new(state.clone());

    tracing::info!("gRPC server listening on {}", addr);

    Server::builder()
        .add_service(DiscoveryServiceServer::new(discovery))
        .add_service(HandshakeServiceServer::new(handshake))
        .add_service(SettlementServiceServer::new(settlement))
        .serve(addr)
        .await?;

    Ok(())
}

/// Load a signing key from a hex file, or generate and persist a new one.
fn load_or_generate_key(path: &str) -> anyhow::Result<SigningKey> {
    use std::io::{Read, Write};

    if let Ok(mut file) = std::fs::File::open(path) {
        let mut hex_str = String::new();
        file.read_to_string(&mut hex_str)?;
        let bytes = hex::decode(hex_str.trim())?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| anyhow::anyhow!("Key must be 32 bytes"))?;
        return Ok(SigningKey::from_bytes(&arr));
    }

    // Generate a new key
    let key = SigningKey::generate(&mut OsRng);

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(path)?;
    write!(file, "{}", hex::encode(key.as_bytes()))?;
    tracing::info!(path, "Generated new signing key");

    Ok(key)
}
