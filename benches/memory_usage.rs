use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use codesift::CodeSift;

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    for repo in &["fixtures/small", "fixtures/medium"] {
        let id = BenchmarkId::from_parameter(*repo);
        group.bench_with_input(id, repo, |b, &repo| {
            b.iter(|| {
                let cs = CodeSift::open(black_box(repo)).unwrap();
                let sym_count = cs.symbol_count();
                let file_count = cs.file_count();
                black_box((sym_count, file_count));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, benchmark_memory_usage);
criterion_main!(benches);
