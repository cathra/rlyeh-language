//! W4（2026-08-25）：Future 版 `join_all` 集成测试。
//!
//! `future::join_all<F: Future>(Vec<F>) -> Vec<i64>`（MVP：输出限 i64）——
//! 并发轮询多个 future 直至全部 `Ready`，按传入顺序收集结果。
//!
//! 需要系统 clang（与 thread_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-jf-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("W4 join_all 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// `future::join_all`：并发轮询多个 future 直至全部 Ready，按序收集结果。
/// 每个 future 3 次 poll 后 Ready(n)，两个 future 输出 [3, 5]。
#[test]
fn join_all_collects_all_results_in_order() {
    let out = run(
        r#"
struct MyFut { n: i64 }
impl Future for MyFut {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        self.n = self.n + 1;
        if self.n >= 3 {
            Poll::Ready(self.n)
        } else {
            Poll::Pending
        }
    }
}
fn main() {
    let fs: Vec<MyFut> = vec![MyFut { n: 2 }, MyFut { n: 4 }];
    let r = future::join_all(fs);
    for x in r {
        println(x);
    }
}
"#,
    );
    assert_eq!(out, "3\n5\n");
}

/// `future::join_all`：立即 `Ready` 的 future（一次 poll），输出即收集值。
#[test]
fn join_all_immediate_futures() {
    let out = run(
        r#"
struct Quick { v: i64 }
impl Future for Quick {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        Poll::Ready(self.v)
    }
}
fn main() {
    let fs: Vec<Quick> = vec![Quick { v: 10 }, Quick { v: 20 }, Quick { v: 30 }];
    let r = future::join_all(fs);
    let mut s = 0;
    for x in r {
        s = s + x;
    }
    println(s);
}
"#,
    );
    assert_eq!(out, "60\n");
}

/// `future::join_all`：返回空 Vec（无 future）→ 空结果。
#[test]
fn join_all_empty() {
    let out = run(
        r#"
struct Quick { v: i64 }
impl Future for Quick {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        Poll::Ready(self.v)
    }
}
fn main() {
    let fs: Vec<Quick> = Vec::new();
    let r = future::join_all(fs);
    println(r.len());
}
"#,
    );
    assert_eq!(out, "0\n");
}

/// W4 补全：`join_all` 返回 `Vec<F::Output>` 支持非 i64 类型（String）——
/// `F::Output` 关联类型投影落地，输出不再限 i64。
#[test]
fn join_all_supports_non_i64_output() {
    let out = run(
        r#"
struct StrFut { s: String, n: i64 }
impl Future for StrFut {
    type Output = String;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        self.n = self.n + 1;
        if self.n >= 2 {
            Poll::Ready(self.s)
        } else {
            Poll::Pending
        }
    }
}
fn main() {
    let fs: Vec<StrFut> = vec![
        StrFut { s: String::from("hi"), n: 0 },
        StrFut { s: String::from("yo"), n: 0 },
    ];
    let r = future::join_all(fs);
    for x in r {
        println(x);
    }
}
"#,
    );
    assert_eq!(out, "hi\nyo\n");
}
