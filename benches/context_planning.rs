use codesift::{CodeSift, TokenBudget};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn benchmark_context_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_planning");

    for repo in &["fixtures/small", "fixtures/medium"] {
        let cs = CodeSift::open(repo).unwrap();

        for query in &["add", "format", "handle"] {
            let id = BenchmarkId::new(*query, *repo);
            group.bench_with_input(id, repo, |b, _| {
                b.iter(|| {
                    let _ = cs.plan_context(black_box(query), Some(TokenBudget::new(2000)));
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, benchmark_context_planning);
criterion_main!(benches);
