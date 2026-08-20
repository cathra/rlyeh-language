//! zeta-driver 集成测试：完整流水线 + clang 汇编 / 运行（需要系统 clang）。

use zeta_driver::{build_executable, compile_to_llvm, run_source};

const HELLO_WORLD: &str = r#"
fn main() {
    println("Hello, Zeta!");
}
"#;

const ARITH_PRINT: &str = r#"
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    let x = 6;
    let y = 7;
    let sum = add(x, y);
    println(sum);
    println("6 * 7 = ");
    println(x * y);
    println();
    if sum > 10 {
        println("sum is big");
    };
    println(true);
    println(false);
}
"#;

#[test]
fn compile_hello_world_llvm() {
    let ll = compile_to_llvm(HELLO_WORLD).expect("编译失败");
    eprintln!("--- LLVM IR ---\n{ll}\n--- END ---");
    assert!(ll.contains("declare i32 @printf(i8*, ...)"));
    assert!(ll.contains("define i32 @main()"));
    assert!(ll.contains("Hello, Zeta!"));
    assert!(ll.contains("@.fmt."));
}

#[test]
fn compile_arith_llvm() {
    let ll = compile_to_llvm(ARITH_PRINT).expect("编译失败");
    eprintln!("--- LLVM IR ---\n{ll}\n--- END ---");
    assert!(ll.contains("define i32 @main()"));
    // add 为单块小函数，会被内联 pass 展开；比较指令（if sum > 10）应保留
    assert!(ll.contains("icmp sgt"));
    assert!(ll.contains("@.str."));
}

#[test]
fn run_hello_world() {
    let out = run_source(HELLO_WORLD).expect("运行失败");
    assert_eq!(out, "Hello, Zeta!\n");
}

#[test]
fn run_arith_print() {
    let out = run_source(ARITH_PRINT).expect("运行失败");
    assert_eq!(out, "13\n6 * 7 = \n42\n\nsum is big\ntrue\nfalse\n");
}

#[test]
fn build_hello_world_executable() {
    let dir = std::env::temp_dir();
    let exe = dir.join(format!(
        "zeta-test-{}-{}.exe",
        std::process::id(),
        rand_suffix()
    ));
    let result = build_executable(HELLO_WORLD, &exe);
    assert!(result.is_ok(), "编译失败: {result:?}");
    assert!(exe.exists());
    let _ = std::fs::remove_file(exe);
}

#[test]
fn run_rejects_type_error() {
    let src = "fn main() { let x = \"str\"; x + 1; }";
    let err = run_source(src).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("typecheck") || msg.contains("borrowck") || msg.contains("regionck"),
        "意外错误: {msg}"
    );
}

/// 简单的随机后缀（避免测试并发冲突）。
fn rand_suffix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}
