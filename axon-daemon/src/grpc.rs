use tonic::{Request, Response, Status};
use axon_v1::discovery_service_server::DiscoveryService;
use axon_v1::handshake_service_server::HandshakeService;
use axon_v1::settlement_service_server::SettlementService;
use axon_v1::{
    PublishManifestRequest, PublishManifestResponse, DiscoverRequest, DiscoverResponse,
    RankedAgent, AgentManifest as ProtoManifest, HandshakeRequest as ProtoHandshakeRequest,
    HandshakeResponse as ProtoHandshakeResponse, HandshakeStatus as ProtoHandshakeStatus,
    SettlementRequest, SettlementResponse,
};

// ... (existing DiscoveryImpl and HandshakeImpl)

pub struct SettlementImpl {
    state: Arc<DaemonState>,
}

impl SettlementImpl {
    pub fn new(state: Arc<DaemonState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl SettlementService for SettlementImpl {
    async fn settle(
        &self,
        request: Request<SettlementRequest>,
    ) -> Result<Response<SettlementResponse>, Status> {
        let req = request.into_inner();
        
        let session_id: [u8; 16] = req.session_id.try_into()
            .map_err(|_| Status::invalid_argument("Invalid session_id"))?;

        match self.state.settlement.settle_session(
            session_id,
            req.actual_compute_units,
            req.agreed_price_per_cu,
        ).await {
            Ok(_) => Ok(Response::new(SettlementResponse {
                success: true,
                tx_digest: "stub_digest".into(),
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
use crate::DaemonState;
use std::sync::Arc;
use axon_core::{AgentManifest, HandshakeRequest, HandshakeEngine};
use crate::dht::Dht;
use ed25519_dalek::Signer;

pub mod axon_v1 {
    tonic::include_proto!("axon.v1");
}

pub struct DiscoveryImpl {
    state: Arc<DaemonState>,
}

impl DiscoveryImpl {
    pub fn new(state: Arc<DaemonState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl DiscoveryService for DiscoveryImpl {
    // ... (unchanged DiscoveryService implementation)
    async fn publish_manifest(
        &self,
        request: Request<PublishManifestRequest>,
    ) -> Result<Response<PublishManifestResponse>, Status> {
        let req = request.into_inner();
        let proto = match req.manifest {
            Some(m) => m,
            None => return Err(Status::invalid_argument("Missing manifest")),
        };

        let manifest = AgentManifest {
            agent_id: proto.agent_id.try_into().map_err(|_| Status::invalid_argument("Invalid agent_id"))?,
            public_key: proto.public_key.try_into().map_err(|_| Status::invalid_argument("Invalid public_key"))?,
            capability_embedding: proto.capability_embedding.iter().map(|&x| x as i8).collect(),
            capability_tags: proto.capability_tags,
            latency_sla_us: proto.latency_sla_us,
            base_price_per_cu: proto.base_price_per_cu,
            staked_amount: proto.staked_amount,
            zk_capability_proof: proto.zk_capability_proof.try_into().map_err(|_| Status::invalid_argument("Invalid zk_proof"))?,
            timestamp_ns: proto.timestamp_ns as u128,
            signature: proto.signature.try_into().map_err(|_| Status::invalid_argument("Invalid signature"))?,
        };

        // Publish to DHT
        self.state.dht.publish(&manifest)
            .map_err(|e| Status::internal(e))?;

        // Update local index (recover from poisoned lock gracefully)
        self.state.index.write()
            .unwrap_or_else(|e| e.into_inner())
            .upsert(manifest);

        Ok(Response::new(PublishManifestResponse {
            accepted: true,
            reason: String::new(),
        }))
    }

    async fn discover(
        &self,
        request: Request<DiscoverRequest>,
    ) -> Result<Response<DiscoverResponse>, Status> {
        let req = request.into_inner();
        
        let embedding: Vec<i8> = req.requirement_embedding.iter().map(|&x| x as i8).collect();
        
        // Use dummy reputation for now (0.5 for everyone)
        let reputation_fn = |_id: &[u8; 32]| 500_000u64;

        let results = self.state.index.read()
            .unwrap_or_else(|e| e.into_inner())
            .search(
            &embedding,
            req.top_k as usize,
            10_000, // median latency placeholder
            reputation_fn,
        );

        let proto_results = results.into_iter().map(|r| {
            RankedAgent {
                rank_score: r.rank_score,
                capability_sim: r.similarity,
                manifest: Some(ProtoManifest {
                    agent_id: r.manifest.agent_id.to_vec(),
                    public_key: r.manifest.public_key.to_vec(),
                    capability_embedding: r.manifest.capability_embedding.iter().map(|&x| x as u8).collect(),
                    capability_tags: r.manifest.capability_tags,
                    latency_sla_us: r.manifest.latency_sla_us,
                    base_price_per_cu: r.manifest.base_price_per_cu,
                    staked_amount: r.manifest.staked_amount,
                    zk_capability_proof: r.manifest.zk_capability_proof.to_vec(),
                    timestamp_ns: r.manifest.timestamp_ns as u64,
                    signature: r.manifest.signature.to_vec(),
                }),
            }
        }).collect();

        Ok(Response::new(DiscoverResponse {
            results: proto_results,
        }))
    }
}

pub struct HandshakeImpl {
    state: Arc<DaemonState>,
}

impl HandshakeImpl {
    pub fn new(state: Arc<DaemonState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl HandshakeService for HandshakeImpl {
    async fn handshake(
        &self,
        request: Request<ProtoHandshakeRequest>,
    ) -> Result<Response<ProtoHandshakeResponse>, Status> {
        let req = request.into_inner();

        let core_req = HandshakeRequest {
            session_id: req.session_id.try_into().map_err(|_| Status::invalid_argument("Invalid session_id"))?,
            initiator_id: req.initiator_id.try_into().map_err(|_| Status::invalid_argument("Invalid initiator_id"))?,
            target_id: req.target_id.try_into().map_err(|_| Status::invalid_argument("Invalid target_id"))?,
            required_capability_embedding: req.required_capability_embedding.iter().map(|&x| x as i8).collect(),
            task_payload_hash: req.task_payload_hash.try_into().map_err(|_| Status::invalid_argument("Invalid task_payload_hash"))?,
            max_compute_units: req.max_compute_units,
            max_price_per_cu: req.max_price_per_cu,
            escrow_amount: req.escrow_amount,
            escrow_object_id: req.escrow_object_id.try_into().map_err(|_| Status::invalid_argument("Invalid escrow_object_id"))?,
            deadline_ns: req.deadline_ns as u128,
            initiator_signature: req.initiator_signature.try_into().map_err(|_| Status::invalid_argument("Invalid signature"))?,
        };

        // 1. Resolve initiator manifest from DHT
        let initiator_manifest = self.state.dht.get(&core_req.initiator_id)
            .ok_or_else(|| Status::not_found("Initiator manifest not found in DHT"))?;

        // 2. Verify escrow on-chain
        let escrow_verified = self.state.monitor.is_verified(&core_req.escrow_object_id).await;

        // 3. Run handshake engine
        let validation = self.state.engine.validate_request(
            &core_req,
            &initiator_manifest,
            escrow_verified,
            0.0, // capacity placeholder
        );

        let (status, agreed_price, commitment) = match validation {
            Ok(v) => {
                let (preimage, comm) = HandshakeEngine::generate_commitment(
                    &v.session_id,
                    &core_req.task_payload_hash,
                    &self.state.manifest.agent_id,
                    v.agreed_price_per_cu,
                );
                
                // Persist preimage to storage before responding
                if let Err(e) = self.state.storage.store_preimage(&v.session_id, &preimage) {
                    tracing::error!(error = %e, "Failed to persist preimage");
                    return Err(Status::internal("Storage failure"));
                }

                (ProtoHandshakeStatus::Accepted, v.agreed_price_per_cu, comm)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Handshake rejected");
                (ProtoHandshakeStatus::Rejected, 0, [0u8; 32])
            }
        };

        let mut res = ProtoHandshakeResponse {
            session_id: core_req.session_id.to_vec(),
            responder_id: self.state.manifest.agent_id.to_vec(),
            status: status as i32,
            agreed_price_per_cu: agreed_price,
            commitment_hash: commitment.to_vec(),
            timestamp_ns: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64,
            responder_signature: vec![0u8; 64],
        };

        // Sign response
        let mut hasher = blake3::Hasher::new();
        hasher.update(&res.session_id);
        hasher.update(&res.responder_id);
        hasher.update(&(res.status as i32).to_le_bytes());
        hasher.update(&res.agreed_price_per_cu.to_le_bytes());
        hasher.update(&res.commitment_hash);
        hasher.update(&res.timestamp_ns.to_le_bytes());
        
        let sig = self.state.key.sign(hasher.finalize().as_bytes());
        res.responder_signature = sig.to_bytes().to_vec();

        Ok(Response::new(res))
    }
}
