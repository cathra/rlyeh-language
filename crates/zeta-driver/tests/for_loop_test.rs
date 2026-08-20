//! for 循环集成测试：range 迭代器 desugar 为 while 的端到端验证。
//!
//! 覆盖：半开/闭/开区间、break/continue、嵌套 for、变量边界、compile-fail 场景。

use zeta_driver::run_source;

const SUM_HALF_OPEN: &str = r#"
fn main() {
    let mut sum = 0;
    for i in 0..<10 {
        sum += i;
    }
    println(sum);
}
"#;

const SUM_CLOSED: &str = r#"
fn main() {
    let mut sum = 0;
    for i in 0...5 {
        sum += i;
    }
    println(sum);
}
"#;

const SUM_OPEN_LOWER: &str = r#"
fn main() {
    // 0<..5 = (0, 5]，元素 1,2,3,4,5
    let mut sum = 0;
    for i in 0<..5 {
        sum += i;
    }
    println(sum);
}
"#;

const SUM_OPEN_BOTH: &str = r#"
fn main() {
    // 双开区间语法上不存在（range 仅 `..<`/`...`/`<..` 三种），
    // 数学上的 (0, 5) 用 `1..<5` 表达：元素 1,2,3,4
    let mut sum = 0;
    for i in 1..<5 {
        sum += i;
    }
    println(sum);
}
"#;

const BREAK_CONTINUE: &str = r#"
fn main() {
    let mut evens = 0;
    for i in 0..<100 {
        if i >= 6 { break; }
        if i % 2 != 0 { continue; }
        evens += i;
    }
    println(evens);
}
"#;

const NESTED_FOR: &str = r#"
fn main() {
    let mut product = 0;
    for a in 1..<4 {
        for b in 1..<4 {
            product += a * b;
        }
    }
    println(product);
}
"#;

const VARIABLE_BOUNDS: &str = r#"
fn main() {
    let n = 7;
    let mut s = 0;
    for i in 0..<n {
        s += i;
    }
    println(s);
}
"#;

const DECREASING_RANGE: &str = r#"
fn main() {
    let mut s = 0;
    for i in 10..<3 {
        s += i;
    }
    println(s);
}
"#;

#[test]
fn for_half_open_sum() {
    assert_eq!(run_source(SUM_HALF_OPEN).expect("运行失败"), "45\n");
}

#[test]
fn for_closed_range_sum() {
    assert_eq!(run_source(SUM_CLOSED).expect("运行失败"), "15\n");
}

#[test]
fn for_open_lower_sum() {
    assert_eq!(run_source(SUM_OPEN_LOWER).expect("运行失败"), "15\n");
}

#[test]
fn for_open_both_sum() {
    assert_eq!(run_source(SUM_OPEN_BOTH).expect("运行失败"), "10\n");
}
#[test]
fn for_break_continue() {
    assert_eq!(run_source(BREAK_CONTINUE).expect("运行失败"), "6\n");
}

#[test]
fn for_nested() {
    assert_eq!(run_source(NESTED_FOR).expect("运行失败"), "36\n");
}

#[test]
fn for_variable_bounds() {
    assert_eq!(run_source(VARIABLE_BOUNDS).expect("运行失败"), "21\n");
}

#[test]
fn for_decreasing_range_is_empty() {
    assert_eq!(run_source(DECREASING_RANGE).expect("运行失败"), "0\n");
}

#[test]
fn for_rejects_string_iterator() {
    let src = r#"
fn main() {
    for ch in "hello" {
        println(ch);
    }
}
"#;
    let err = run_source(src).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("typecheck") || msg.contains("不支持") || msg.contains("for"),
        "意外错误: {msg}"
    );
}
