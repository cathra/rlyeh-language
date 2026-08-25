//! D3 集成测试：`rlyeh doc`（/// 文档提取）与 `rlyeh bench`（基准计时）在
//! driver 层的可用性验证。

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rlyeh-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------- rlyeh-doc ----------

const DOC_SOURCE: &str = r#"
//! 文件级说明：一个示例模块。

/// 计算两数之和。
/// 第二行说明。
pub fn add(a: i64, b: i64) -> i64 { a + b }

/// 点结构体。
struct Point { x: i64, y: i64 }

/// 计数器。
actor Counter {
    value: i64 = 0,
}

/// 形状枚举。
enum Shape { Circle(f64), Rect { w: f64, h: f64 } }
"#;

#[test]
fn doc_extracts_comments_and_signatures() {
    let md = rlyeh_doc::doc_source(DOC_SOURCE, &rlyeh_doc::DocOptions::default()).unwrap();
    // 文件级注释
    assert!(md.contains("文件级说明：一个示例模块。"));
    // 项注释与签名
    assert!(md.contains("计算两数之和。"));
    assert!(md.contains("第二行说明。"));
    assert!(md.contains("pub fn add(a: i64, b: i64) -> i64"));
    assert!(md.contains("点结构体。"));
    assert!(md.contains("struct Point { x: i64, y: i64 }"));
    assert!(md.contains("计数器。"));
    assert!(md.contains("actor Counter"));
    assert!(md.contains("形状枚举。"));
    assert!(md.contains("enum Shape"));
    // 位置信息（add 位于第 6 行，`pub` 修饰符后 `fn` 关键字起始列 5）
    assert!(md.contains("> 位置: 6:5"));
    // 目录
    assert!(md.contains("## 目录"));
}

#[test]
fn doc_lists_items_without_comments() {
    let md =
        rlyeh_doc::doc_source("fn foo() {}\nstruct Bar {}\n", &rlyeh_doc::DocOptions::default())
            .unwrap();
    assert!(md.contains("## 函数"));
    assert!(md.contains("fn foo()"));
    assert!(md.contains("## 结构体"));
    assert!(md.contains("struct Bar"));
}

#[test]
fn doc_parse_error_reported() {
    assert!(rlyeh_doc::doc_source("fn broken(", &rlyeh_doc::DocOptions::default()).is_err());
}

#[test]
fn doc_driver_file_api() {
    let dir = tmp_dir("doc");
    let f = dir.join("docme.rl");
    std::fs::write(&f, "/// 外部函数。\nextern fn putchar(c: i32);\nfn main() {}\n").unwrap();
    let md = rlyeh_driver::doc_source_file(&f, &rlyeh_doc::DocOptions::default()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(md.contains("外部函数。"));
    assert!(md.contains("extern fn putchar(c: i32)"));
}

#[test]
fn doc_driver_file_api_parse_error() {
    let dir = tmp_dir("doc-err");
    let f = dir.join("bad.rl");
    std::fs::write(&f, "fn broken(\n").unwrap();
    let res = rlyeh_driver::doc_source_file(&f, &rlyeh_doc::DocOptions::default());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(res.is_err());
}

// ---------- rlyeh-bench ----------

#[test]
fn bench_report_statistics_sanity() {
    // 直接构造耗时数据验证统计计算（不启动进程）
    let report = rlyeh_bench::BenchReport::from_measurements(vec![0.01, 0.02, 0.03]);
    assert_eq!(report.measurements.len(), 3);
    assert!((report.mean - 0.02).abs() < 1e-12);
    assert!((report.median - 0.02).abs() < 1e-12);
    assert!(report.min <= report.median && report.median <= report.max);
    assert!(report.stddev > 0.0);
    assert!(report.ops_per_sec > 0.0);
    let text = format!("{report}");
    assert!(text.contains("平均") && text.contains("中位数") && text.contains("吞吐"));
}

#[test]
fn bench_times_compiled_executable() {
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.rl");
    assert!(hello.exists(), "缺测试用例: {}", hello.display());
    let dir = tmp_dir("bench");
    let exe = dir.join("hello-bench");
    rlyeh_driver::build_executable_file(&hello, &exe).unwrap();
    let opts = rlyeh_bench::BenchOptions {
        warmup: 1,
        runs: 3,
        quiet: true,
    };
    let report = rlyeh_bench::bench_executable(&exe, &opts).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(report.measurements.len(), 3);
    assert!(report.mean > 0.0);
    assert!(report.ops_per_sec > 0.0);
}

#[test]
fn bench_compile_and_time_source() {
    let root = workspace_root();
    let hello = root.join("tests/run-pass/hello.rl");
    let dir = tmp_dir("bench-src");
    let exe = dir.join("hello-bench");
    let opts = rlyeh_bench::BenchOptions {
        warmup: 1,
        runs: 3,
        quiet: true,
    };
    let report = rlyeh_bench::bench_source(&hello, &exe, &opts, |src, out| {
        rlyeh_driver::build_executable_file(src, out).map_err(|e| e.to_string())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(report.measurements.len(), 3);
    assert!(report.mean > 0.0);
}

#[test]
fn bench_missing_executable_is_error() {
    let res = rlyeh_bench::bench_executable(
        Path::new("/nonexistent/rlyeh-bench-bin"),
        &rlyeh_bench::BenchOptions::default(),
    );
    assert!(res.is_err());
}
