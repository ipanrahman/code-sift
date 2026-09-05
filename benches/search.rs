use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use codesift::CodeSift;

fn benchmark_search(c: &mut Criterion) {
    let cs = CodeSift::open(".").unwrap();

    let mut group = c.benchmark_group("search");

    for query in &["CodeSift", "fn", "struct"] {
        group.bench_function(BenchmarkId::from_parameter(query), |b| {
            b.iter(|| {
                let _ = cs.search(black_box(query), None);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, benchmark_search);
criterion_main!(benches);
