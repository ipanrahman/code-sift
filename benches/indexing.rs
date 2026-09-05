use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use codesift::CodeSift;

fn benchmark_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexing");

    for repo in &["fixtures/small", "fixtures/medium"] {
        let cold_id = BenchmarkId::new("cold", *repo);
        group.bench_with_input(cold_id, repo, |b, &repo| {
            b.iter(|| {
                let _ = CodeSift::open(black_box(repo));
            });
        });

        let reindex_id = BenchmarkId::new("reindex", *repo);
        group.bench_with_input(reindex_id, repo, |b, &repo| {
            let mut cs = CodeSift::open(repo).unwrap();
            b.iter(|| {
                cs.reindex().unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(benches, benchmark_indexing);
criterion_main!(benches);
