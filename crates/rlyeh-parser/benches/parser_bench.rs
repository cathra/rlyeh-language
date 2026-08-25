//! rlyeh-parser 基准测试：不同规模的源码解析耗时。

use criterion::Criterion;
use std::hint::black_box;
use rlyeh_parser::parse;

/// 小型程序（单函数 + 若干语句）
const SMALL_SRC: &str = r#"
fn add(a: u32, b: u32) -> u32 {
    let sum = a + b;
    if sum > 100 {
        return sum;
    }
    sum
}
"#;

/// 中型程序（比较链、集合、match、区域）
const MEDIUM_SRC: &str = r#"
fn classify(x: u32, ch: char, hour: u8) -> u32 {
    let zone = if 0 < x < 10 {
        1
    } else if x in (10, 20, 30) {
        2
    } else if ch in ('a'..<'z', 'A'..<'Z') {
        3
    } else if hour in (9am...6pm) {
        4
    } else {
        5
    };
    match zone {
        1...2 => zone * 2,
        3...4 => zone + 10,
        _ => 0,
    }
}

actor Counter {
    value: u32 = 0,
    pub fn increment(amount: u32) -> u32 {
        self.value += amount;
        self.value
    }
}
"#;

/// 大型程序（region + transfer + 结构体 + trait + impl）
const LARGE_SRC: &str = r#"
struct Point { x: f64, y: f64 }
struct BigStruct { data: [u8; 4096] }

trait Shape {
    fn area(&self) -> f64;
    fn name() -> String;
}

impl Shape for Point {
    fn area(&self) -> f64 { 0.0 }
    fn name() -> String { "point" }
}

fn build() -> BigStruct {
    region 'r with_size(16384) allow_growth(growth_factor=2.0) {
        let data = BigStruct { data: [0; 4096] } in 'r;
        process(&data);
        transfer data out of 'r
    }
}

fn process(data: &BigStruct) -> u32 {
    data.data.len() as u32
}

fn fetch(url: &str, timeout: Duration = Duration::seconds(30)) -> Result<Response, Error> {
    region 'r {
        let client = HttpClient::new() in 'r;
        let resp = client.get(url).await in 'r;
        let _ = resp;
    }
}
"#;

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");
    group.bench_function("small", |b| b.iter(|| parse(black_box(SMALL_SRC))));
    group.bench_function("medium", |b| b.iter(|| parse(black_box(MEDIUM_SRC))));
    group.bench_function("large", |b| b.iter(|| parse(black_box(LARGE_SRC))));
    group.finish();
}

criterion::criterion_group!(benches, bench_parse);
criterion::criterion_main!(benches);
