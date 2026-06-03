//! In-process HNSW index over agent manifests.
//!
//! # MVP implementation
//! This uses a brute-force linear scan as a drop-in placeholder.
//! Replace the `search` implementation with `instant-distance` or a
//! purpose-built HNSW crate once the interface is proven.
//!
//! The interface is identical to production — the swap is one impl block.

use axon_core::{cosine_similarity_i8, composite_rank_score, AgentManifest, EMBEDDING_DIM};
use instant_distance::{Builder, HnswMap, Point, Search};
use std::collections::HashMap;

/// A ranked search result from the ANN index.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub agent_id:       [u8; 32],
    pub rank_score:     f32,
    pub similarity:     f32,
    pub manifest:       AgentManifest,
}

/// Wrapper for i8-quantized embeddings to implement `instant_distance::Point`.
#[derive(Clone, Debug)]
struct EmbeddingPoint(Vec<i8>);

impl Point for EmbeddingPoint {
    fn distance(&self, other: &Self) -> f32 {
        // Cosine distance = 1.0 - cosine_similarity.
        // We use the optimised similarity function from axon-core.
        1.0 - cosine_similarity_i8(&self.0, &other.0)
    }
}

/// HNSW vector index over agent manifests using `instant-distance`.
pub struct HnswIndex {
    entries: HashMap<[u8; 32], AgentManifest>,
    index:   Option<HnswMap<EmbeddingPoint, [u8; 32]>>,
}

impl HnswIndex {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), index: None }
    }

    /// Insert or update an agent manifest.
    ///
    /// Rebuilds the HNSW index on every change (MVP strategy).
    pub fn upsert(&mut self, manifest: AgentManifest) -> bool {
        let id = manifest.agent_id;
        let changed = match self.entries.get(&id) {
            Some(existing) if existing.timestamp_ns >= manifest.timestamp_ns => false,
            _ => { self.entries.insert(id, manifest); true }
        };

        if changed {
            self.rebuild_index();
        }
        changed
    }

    /// Remove an agent manifest by ID.
    pub fn remove(&mut self, agent_id: &[u8; 32]) -> bool {
        let removed = self.entries.remove(agent_id).is_some();
        if removed {
            self.rebuild_index();
        }
        removed
    }

    fn rebuild_index(&mut self) {
        if self.entries.is_empty() {
            self.index = None;
            return;
        }

        let points: Vec<EmbeddingPoint> = self.entries.values()
            .map(|m| EmbeddingPoint(m.capability_embedding.clone()))
            .collect();
        let values: Vec<[u8; 32]> = self.entries.keys().cloned().collect();

        self.index = Some(Builder::default().build(points, values));
    }

    /// Return the top-`k` agents most similar to `query_embedding`.
    pub fn search<F>(
        &self,
        query_embedding: &[i8],
        k: usize,
        global_median_latency_us: u64,
        reputation_fn: F,
    ) -> Vec<SearchResult>
    where
        F: Fn(&[u8; 32]) -> u64,
    {
        assert_eq!(query_embedding.len(), EMBEDDING_DIM);

        let index = match &self.index {
            Some(i) => i,
            None => return Vec::new(),
        };

        let query_point = EmbeddingPoint(query_embedding.to_vec());
        let mut search = Search::default();

        // We fetch 2*k from the ANN to have enough candidates for composite reranking
        let candidates = index.search(&query_point, &mut search);

        let mut scored: Vec<SearchResult> = candidates
            .take(k * 2)
            .filter_map(|item| {
                let m = self.entries.get(item.value)?;
                let sim = 1.0 - item.distance; // distance = 1.0 - sim
                
                if sim < axon_core::CAPABILITY_MATCH_THRESHOLD {
                    return None;
                }

                let rep  = reputation_fn(&m.agent_id) as f32 / 1_000_000.0;
                let lat  = if global_median_latency_us == 0 {
                    0.0
                } else {
                    m.latency_sla_us as f32 / global_median_latency_us as f32
                };
                let rank = composite_rank_score(sim, rep, lat);
                
                Some(SearchResult {
                    agent_id: m.agent_id,
                    rank_score: rank,
                    similarity: sim,
                    manifest: m.clone(),
                })
            })
            .collect();

        scored.sort_by(|a, b| b.rank_score.partial_cmp(&a.rank_score).unwrap());
        scored.truncate(k);
        scored
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}
