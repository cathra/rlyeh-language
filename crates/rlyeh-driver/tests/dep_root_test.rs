//! dagon 包集成（P4）回归：`--dep-root pkg=dir` 将第三方依赖注入为扁平名字空间。
//!
//! 覆盖：`inject_dep_roots` 以 `pub module pkg { ... }` 包裹依赖入口并前置，
//! 主程序经 `import pkg::item;` 访问（依赖经工作流 C 的 `pub` 契约暴露公共面）。

use std::path::{Path, PathBuf};

use rlyeh_driver::IncrementalDriver;

/// 独立临时目录，避免并行测试互相覆盖。
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rlyeh_dep_root_{}_{}_{}",
        name,
        std::process::id(),
        name.len()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 写入一个最小依赖库：`<dir>/src/lib.rl` 含 `pub module api { pub fn foo }` 与 `pub fn answer`。
fn write_demo_dep(root: &Path) -> PathBuf {
    let dep = root.join("depdemo");
    std::fs::create_dir_all(dep.join("src")).unwrap();
    std::fs::write(
        dep.join("src/lib.rl"),
        "pub module api {\n    pub fn foo() -> i64 { 7 }\n}\n\
         pub fn answer() -> i64 { 42 }\n",
    )
    .unwrap();
    dep
}

#[test]
fn dep_root_injection_resolves_pub_module() {
    let base = temp_dir("demo");
    let dep = write_demo_dep(&base);

    let main_src = "import demo::api::foo;\n\
                    import demo::answer;\n\
                    fn main() {\n    println(foo() + answer());\n}\n";

    let mut driver = IncrementalDriver::new(base.clone())
        .with_dep_roots(vec![("demo".to_string(), dep.clone())]);
    let (stdout, _) = driver.run_source("main.rl", main_src).unwrap();

    assert_eq!(stdout, "49\n");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn dep_root_missing_entry_errors() {
    let base = temp_dir("missing");
    let dep = base.join("nonexistent"); // 不存在的依赖根，应报 Module 错误

    let main_src = "fn main() { println(0); }\n";
    let mut driver = IncrementalDriver::new(base.clone())
        .with_dep_roots(vec![("demo".to_string(), dep.clone())]);
    assert!(
        driver.run_source("main.rl", main_src).is_err(),
        "缺失依赖入口应编译失败"
    );
    let _ = std::fs::remove_dir_all(&base);
}
