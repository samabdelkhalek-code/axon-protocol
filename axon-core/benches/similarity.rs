use criterion::{black_box, criterion_group, criterion_main, Criterion};
use axon_core::{cosine_similarity_i8, EMBEDDING_DIM};
use rand::Rng;

fn bench_cosine_similarity(c: &mut Criterion) {
    let mut rng = rand::thread_rng();
    let a: Vec<i8> = (0..EMBEDDING_DIM).map(|_| rng.gen_range(-128..=127)).collect();
    let b: Vec<i8> = (0..EMBEDDING_DIM).map(|_| rng.gen_range(-128..=127)).collect();

    c.benchmark_group("similarity")
        .bench_function("cosine_similarity_i8", |bencher| {
            bencher.iter(|| {
                cosine_similarity_i8(black_box(&a), black_box(&b))
            });
        });
}

criterion_group!(benches, bench_cosine_similarity);
criterion_main!(benches);
