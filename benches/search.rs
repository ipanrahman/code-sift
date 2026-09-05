use codesift::CodeSift;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn benchmark_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_latency");

    for repo in &["fixtures/small", "fixtures/medium"] {
        let cs = CodeSift::open(repo).unwrap();

        for query in &["add", "format", "handle"] {
            let id = BenchmarkId::new(*query, *repo);
            group.bench_with_input(id, query, |b, &query| {
                b.iter(|| {
                    let _ = cs.search(black_box(query), None);
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, benchmark_search);
criterion_main!(benches);
