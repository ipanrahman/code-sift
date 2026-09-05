use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use codesift::{CodeSift, TokenBudget};

fn count_tokens(text: &str) -> usize {
    text.chars().filter(|c| c.is_whitespace()).count() + text.len() / 4
}

fn benchmark_token_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_efficiency");

    for repo in &["fixtures/small", "fixtures/medium"] {
        let cs = CodeSift::open(repo).unwrap();

        for (query, budget_tokens) in &[("add", 500), ("format", 1000)] {
            let id = BenchmarkId::new(format!("{}_{}", query, repo), repo);
            group.bench_with_input(id, &(query, budget_tokens), |b, &(query, budget_tokens)| {
                b.iter(|| {
                    let plan = cs.plan_context(
                        black_box(query),
                        Some(TokenBudget::new(black_box(*budget_tokens))),
                    ).unwrap();
                    let output: usize = plan.fragments.iter().map(|f| count_tokens(&f.content)).sum();
                    black_box(output);
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, benchmark_token_efficiency);
criterion_main!(benches);
