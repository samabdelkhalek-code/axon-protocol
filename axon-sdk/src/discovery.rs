use axon_core::AgentManifest;
use crate::register::axon_v1::discovery_service_client::DiscoveryServiceClient;
use crate::register::axon_v1::DiscoverRequest;

pub struct DiscoverResult {
    pub agents: Vec<RankedAgent>,
}

pub struct RankedAgent {
    pub manifest:   AgentManifest,
    pub rank_score: f32,
    pub similarity: f32,
}

/// Discover agents matching a natural-language requirement.
pub async fn discover(
    requirement: &str,
    top_k:       u32,
) -> anyhow::Result<DiscoverResult> {
    let mut client = DiscoveryServiceClient::connect("http://127.0.0.1:50051").await?;

    // Placeholder for real embedding generation (should call embedding service)
    let embedding = placeholder_embedding(requirement);

    let response = client.discover(DiscoverRequest {
        requirement_embedding: embedding.iter().map(|&x| x as u8).collect(),
        top_k,
        min_reputation_score: 100,
    }).await?;

    let agents = response.into_inner().results.into_iter().map(|r| {
        let proto = r.manifest.unwrap();
        RankedAgent {
            rank_score: r.rank_score,
            similarity: r.capability_sim,
            manifest: AgentManifest {
                agent_id: proto.agent_id.try_into().unwrap(),
                public_key: proto.public_key.try_into().unwrap(),
                capability_embedding: proto.capability_embedding.iter().map(|&x| x as i8).collect(),
                capability_tags: proto.capability_tags,
                latency_sla_us: proto.latency_sla_us,
                base_price_per_cu: proto.base_price_per_cu,
                staked_amount: proto.staked_amount,
                zk_capability_proof: proto.zk_capability_proof.try_into().unwrap(),
                timestamp_ns: proto.timestamp_ns as u128,
                signature: proto.signature.try_into().unwrap(),
            },
        }
    }).collect();

    Ok(DiscoverResult { agents })
}

fn placeholder_embedding(desc: &str) -> Vec<i8> {
    let mut emb = vec![0i8; axon_core::EMBEDDING_DIM];
    for (i, byte) in desc.bytes().enumerate().take(axon_core::EMBEDDING_DIM) {
        emb[i] = (byte as i8).wrapping_sub(64);
    }
    emb
}
