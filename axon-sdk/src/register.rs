//! Agent registration workflow.

use axon_core::{AgentManifest, ManifestBuilder, EMBEDDING_DIM};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use axon_v1::discovery_service_client::DiscoveryServiceClient;
use axon_v1::{PublishManifestRequest, AgentManifest as ProtoManifest};

pub mod axon_v1 {
    tonic::include_proto!("axon.v1");
}

/// Result of a successful agent registration.
pub struct RegisterResult {
    pub agent_id:      [u8; 32],
    pub manifest:      AgentManifest,
    pub signing_key:   SigningKey,
    pub manifest_cid:  String, // BLAKE3 hex of manifest content hash
}

/// Register an agent with the AXON network.
///
/// # Arguments
/// - `capability_description`: Natural-language description of what this agent does.
///   Used to generate the capability embedding.
/// - `price_per_cu`: Base price per compute unit in picoSUI.
/// - `stake_amount`: Performance bond in picoSUI (min = 2 × expected max escrow).
/// - `genesis_hash`: 32-byte genesis block hash for the target network.
///
/// # What this does
/// 1. Generates (or loads) an Ed25519 signing key
/// 2. Generates the capability embedding from `capability_description`
///    (calls the local embedding service — TODO: connect to axond via IPC)
/// 3. Generates the ZK capability proof (TODO: Groth16 prover)
/// 4. Builds and signs the AgentManifest
/// 5. Publishes the manifest to the DHT via axond
pub async fn register(
    capability_description: &str,
    price_per_cu:           u64,
    stake_amount:           u64,
    genesis_hash:           &[u8; 32],
) -> anyhow::Result<RegisterResult> {
    let key = SigningKey::generate(&mut OsRng);

    // TODO: call local embedding service (axond gRPC) to get a real embedding
    let embedding = placeholder_embedding(capability_description);

    // TODO: generate Groth16 ZK proof of embedding derivation
    let zk_proof = vec![0u8; 128];

    let manifest = ManifestBuilder::new()
        .embedding(embedding)
        .tags(extract_tags(capability_description))
        .latency_sla_us(10_000)
        .price_per_cu(price_per_cu)
        .stake(stake_amount)
        .zk_proof(zk_proof)
        .build(&key, genesis_hash)?;

    let cid = hex::encode(manifest.content_hash());

    // 5. Publishes the manifest to the DHT via axond
    let mut client = DiscoveryServiceClient::connect("http://127.0.0.1:50051").await
        .map_err(|e| anyhow::anyhow!("Failed to connect to axond: {}", e))?;

    let proto_manifest = ProtoManifest {
        agent_id: manifest.agent_id.to_vec(),
        public_key: manifest.public_key.to_vec(),
        capability_embedding: manifest.capability_embedding.iter().map(|&x| x as u8).collect(),
        capability_tags: manifest.capability_tags.clone(),
        latency_sla_us: manifest.latency_sla_us,
        base_price_per_cu: manifest.base_price_per_cu,
        staked_amount: manifest.staked_amount,
        zk_capability_proof: manifest.zk_capability_proof.to_vec(),
        timestamp_ns: manifest.timestamp_ns as u64,
        signature: manifest.signature.to_vec(),
    };

    let response = client.publish_manifest(PublishManifestRequest {
        manifest: Some(proto_manifest),
    }).await?;

    if !response.get_ref().accepted {
        return Err(anyhow::anyhow!("Manifest rejected by axond: {}", response.get_ref().reason));
    }

    tracing::info!(
        agent_id  = manifest.short_id(),
        cid       = %cid,
        price_cu  = price_per_cu,
        "Agent registered"
    );

    Ok(RegisterResult {
        agent_id:    manifest.agent_id,
        manifest_cid: cid,
        manifest,
        signing_key: key,
    })
}

/// Deterministic placeholder embedding.
/// Replace with a call to the sentence-transformer embedding service.
fn placeholder_embedding(description: &str) -> Vec<i8> {
    let mut emb = vec![0i8; EMBEDDING_DIM];
    for (i, byte) in description.bytes().enumerate().take(EMBEDDING_DIM) {
        emb[i] = (byte as i8).wrapping_sub(64);
    }
    emb
}

fn extract_tags(description: &str) -> Vec<String> {
    description
        .split_whitespace()
        .filter(|w| w.len() > 4)
        .take(8)
        .map(|w| w.to_lowercase().trim_matches(|c: char| !c.is_alphabetic()).to_string())
        .filter(|w| !w.is_empty())
        .collect()
}
