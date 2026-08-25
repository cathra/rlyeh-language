//! rlyeh-lexer 基准测试。
//!
//! 性能目标：10K 行代码 < 5ms，吞吐量 > 50MB/s。

use criterion::Criterion;
use std::hint::black_box;
use rlyeh_lexer::Lexer;

const SMALL_SOURCE: &str = "let x = 42; if x > 0 { println!(\"{}\", x); }";

/// 生成 N 行模拟代码
fn generate_source(lines: usize) -> String {
    let line = "let foo_bar = 123 + 456; if x < 10 { y = x * 2; } // comment\n";
    let mut src = String::with_capacity(line.len() * lines);
    for _ in 0..lines {
        src.push_str(line);
    }
    src
}

fn bench_small_file(c: &mut Criterion) {
    c.bench_function("lex_small", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(SMALL_SOURCE));
            lexer.tokenize().unwrap()
        })
    });
}

fn bench_10k_lines(c: &mut Criterion) {
    let source = generate_source(10_000);
    c.bench_function("lex_10k_lines", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(&source));
            lexer.tokenize().unwrap()
        })
    });
}

fn bench_100k_lines(c: &mut Criterion) {
    let source = generate_source(100_000);
    c.bench_function("lex_100k_lines", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(&source));
            lexer.tokenize().unwrap()
        })
    });
}

criterion::criterion_group!(benches, bench_small_file, bench_10k_lines, bench_100k_lines);
criterion::criterion_main!(benches);
