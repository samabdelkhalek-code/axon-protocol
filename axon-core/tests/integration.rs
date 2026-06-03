//! Integration tests for axon-core.
//!
//! These tests exercise the full handshake lifecycle end-to-end
//! without any network or on-chain calls.

use axon_core::{
    HandshakeEngine, HandshakeRequest, ManifestBuilder, EMBEDDING_DIM,
};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn gen_key() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

fn make_manifest(key: &SigningKey, price: u64, emb_val: i8) -> axon_core::AgentManifest {
    ManifestBuilder::new()
        .embedding(vec![emb_val; EMBEDDING_DIM])
        .tags(vec!["text-summarisation".into(), "rag-retrieval".into()])
        .latency_sla_us(10_000)
        .price_per_cu(price)
        .stake(200_000_000)
        .build(key, &[0u8; 32])
        .expect("manifest build should succeed")
}

fn make_signed_request(
    initiator_key: &SigningKey,
    initiator_manifest: &axon_core::AgentManifest,
    target_id: [u8; 32],
    emb_val: i8,
    price_floor: u64,
    deadline_ns: u128,
) -> HandshakeRequest {
    let max_cu = 500u64;
    let max_price = price_floor * 2;

    let mut req = HandshakeRequest {
        session_id:                    rand::random(),
        initiator_id:                  initiator_manifest.agent_id,
        target_id,
        required_capability_embedding: vec![emb_val; EMBEDDING_DIM],
        task_payload_hash:             blake3::hash(b"summarise this document").into(),
        max_compute_units:             max_cu,
        max_price_per_cu:              max_price,
        escrow_amount:                 max_cu * max_price,
        escrow_object_id:              [0xABu8; 32],
        deadline_ns,
        initiator_signature:           vec![0u8; 64],
    };

    let sig = initiator_key.sign(&req.content_hash());
    req.initiator_signature = sig.to_bytes().to_vec();
    req
}

#[test]
fn full_handshake_lifecycle() {
    let initiator_key = gen_key();
    let responder_key = gen_key();

    let initiator_manifest = make_manifest(&initiator_key, 100, 100i8);
    let responder_manifest  = make_manifest(&responder_key, 100, 100i8);

    // Step 1: Responder node creates its engine
    let engine = HandshakeEngine {
        local_agent_id: responder_manifest.agent_id,
        local_manifest: responder_manifest.clone(),
    };

    // Step 2: Initiator builds and signs a request
    let req = make_signed_request(
        &initiator_key,
        &initiator_manifest,
        responder_manifest.agent_id,
        100i8,          // same embedding family → high cosine similarity
        100,            // floor price
        now_ns() + 60_000_000_000, // 60s deadline
    );

    // Step 3: Daemon passes request to engine (escrow assumed verified)
    let result = engine.validate_request(&req, &initiator_manifest, true, 0.0);
    assert!(result.is_ok(), "Validation should succeed: {:?}", result);

    let vr = result.unwrap();
    assert_eq!(vr.agreed_price_per_cu, 100);
    assert!(vr.capability_match_score > 0.82);

    // Step 4: Engine generates commitment for this session
    let (preimage, commitment_hash) = HandshakeEngine::generate_commitment(
        &req.session_id,
        &req.task_payload_hash,
        &engine.local_agent_id,
        vr.agreed_price_per_cu,
    );

    // Step 5: Verify preimage → commitment_hash relationship
    let recomputed = *blake3::Hasher::new()
        .update(&preimage)
        .finalize()
        .as_bytes();
    assert_eq!(recomputed, commitment_hash, "Preimage must hash to commitment");

    println!(
        "✓ Handshake succeeded — session: {}, similarity: {:.4}, price: {} picoSUI/CU",
        hex::encode(&req.session_id[..8]),
        vr.capability_match_score,
        vr.agreed_price_per_cu
    );
}

#[test]
fn manifest_signature_is_verifiable() {
    let key = gen_key();
    let manifest = make_manifest(&key, 500, 50i8);
    assert!(manifest.verify_signature().is_ok());
}

#[test]
fn tampered_manifest_signature_fails() {
    let key = gen_key();
    let mut manifest = make_manifest(&key, 500, 50i8);
    // Tamper with the price after signing
    manifest.base_price_per_cu = 1;
    assert!(manifest.verify_signature().is_err());
}

#[test]
fn price_oracle_circuit_breaker() {
    let mut oracle = axon_core::PriceOracle::new(1_000);
    // After feeding 50 identical samples, EMA ≈ initial
    for _ in 0..50 {
        oracle.update(1_000);
    }
    // 10× threshold
    assert!(oracle.is_above_circuit_breaker(10_001));
    assert!(!oracle.is_above_circuit_breaker(9_999));
}
