//! zeta-driver 多文件模块集成测试：`mod foo;` 外部模块自动加载。
//!
//! 运行完整流水线需要系统 clang（汇编 / 链接）。

use std::path::PathBuf;

use zeta_driver::{compile_file_to_llvm, run_source_file};

/// 临时项目目录（自动清理）。
struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "zeta-mod-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::create_dir_all(&root).unwrap();
        TempProject { root }
    }

    /// 写入一个相对路径文件（自动创建父目录）。
    fn write(&self, rel: &str, content: &str) {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    /// 入口文件绝对路径。
    fn entry(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 进程内唯一后缀（原子计数器），避免并行测试间临时目录冲突。
fn rand_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{c}", std::process::id())
}

/// 基本多文件：入口 + `math.zeta` 子模块，跨模块函数调用。
#[test]
fn compile_module_llvm() {
    let p = TempProject::new();
    p.write(
        "main.zeta",
        r#"
mod math;
use math::add;
fn main() {
    println(add(2, 3));
}
"#,
    );
    p.write(
        "math.zeta",
        r#"
pub fn add(a: i64, b: i64) -> i64 { a + b }
"#,
    );
    let ll = compile_file_to_llvm(&p.entry("main.zeta")).expect("编译失败");
    assert!(ll.contains("define i32 @main()"));
    // 组合源码应把子模块展开为内联 mod（typecheck 扁平符号名 math::add）
    assert!(ll.contains("math::add"));
}

/// 运行多文件程序。
#[test]
fn run_module_basic() {
    let p = TempProject::new();
    p.write(
        "main.zeta",
        r#"
mod math;
use math::add;
fn main() {
    println(add(2, 3));
}
"#,
    );
    p.write(
        "math.zeta",
        r#"
pub fn add(a: i64, b: i64) -> i64 { a + b }
"#,
    );
    let out = run_source_file(&p.entry("main.zeta")).expect("运行失败");
    assert_eq!(out, "5\n");
}

/// use 别名跨模块调用。
#[test]
fn run_module_alias() {
    let p = TempProject::new();
    p.write(
        "main.zeta",
        r#"
mod math;
use math::mul as times;
fn main() {
    println(times(6, 7));
}
"#,
    );
    p.write(
        "math.zeta",
        r#"
pub fn mul(a: i64, b: i64) -> i64 { a * b }
"#,
    );
    let out = run_source_file(&p.entry("main.zeta")).expect("运行失败");
    assert_eq!(out, "42\n");
}

/// 目录形式子模块：`<name>/mod.zeta`。
#[test]
fn run_module_dir_form() {
    let p = TempProject::new();
    p.write(
        "main.zeta",
        r#"
mod foo;
use foo::bar;
fn main() {
    println(bar());
}
"#,
    );
    p.write(
        "foo/mod.zeta",
        r#"
pub fn bar() -> i64 { 42 }
"#,
    );
    let out = run_source_file(&p.entry("main.zeta")).expect("运行失败");
    assert_eq!(out, "42\n");
}

/// 两级嵌套模块：`a.zeta` 内部再声明 `pub mod b;`。
#[test]
fn run_module_nested() {
    let p = TempProject::new();
    p.write(
        "main.zeta",
        r#"
mod a;
use a::b::deep;
fn main() {
    println(deep());
}
"#,
    );
    p.write(
        "a.zeta",
        r#"
pub mod b;
"#,
    );
    p.write(
        "a/b.zeta",
        r#"
pub fn deep() -> i64 { 7 }
"#,
    );
    let out = run_source_file(&p.entry("main.zeta")).expect("运行失败");
    assert_eq!(out, "7\n");
}

/// 子模块缺失报错。
#[test]
fn module_missing_error() {
    let p = TempProject::new();
    p.write(
        "main.zeta",
        r#"
mod nope;
fn main() { println(1); }
"#,
    );
    let err = run_source_file(&p.entry("main.zeta")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("nope"), "错误应提及模块名: {msg}");
    assert!(msg.contains("mod.zeta"), "错误应提示查找路径: {msg}");
}

/// 模块循环引用报错（通过 symlink 使 `a.zeta` 指回入口文件触发）。
#[cfg(unix)]
#[test]
fn module_cycle_error() {
    use std::os::unix::fs::symlink;

    let p = TempProject::new();
    p.write(
        "main.zeta",
        r#"
mod a;
use a::f;
fn main() { println(f()); }
"#,
    );
    // `a.zeta` 软链接回入口：展开时 canonicalize 命中已访问文件 → 循环
    symlink(p.entry("main.zeta"), p.entry("a.zeta")).unwrap();
    let err = run_source_file(&p.entry("main.zeta")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("循环"), "错误应提及循环引用: {msg}");
}
