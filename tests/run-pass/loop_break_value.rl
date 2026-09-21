// `break <value>`：`loop` 表达式的值由 `break` 携带（EH-6 M1 前置，2026-09-21）。
//
// 背景：`break e` 的 AST / HIR 一直保留值，但 **typecheck** 把 `loop` 恒定为
// `Type::Never`（`check_expr/ctrl.rs`），**MIR** 在降低 `break` 时显式丢弃值
// （`crates/rlyeh-mir/src/lower.rs`：「break 携带的值 MVP 阶段丢弃」）且出口块
// 恒赋 `Unit`——`let x = loop { break 5 };` 中 `x` 无值。现两处补齐：
//   ① typecheck 以「循环 break 值类型栈」把 `break [e]` 的类型登记到最内层循环，
//      `loop` 表达式类型即该值（无 `break` → `Never`，`break;` → `()`）；
//   ② MIR 的 `LoopCtx` 增加 `break_value` 承载槽，带值 `break` 写入该槽后跳出口块，
//      出口块以该槽为循环表达式的值。
//
// 说明：`while` 循环的 `break` 无值（循环类型恒为 `()`，对齐 Rust），其 `break`
// 不写入任何槽——但循环上下文仍压栈占位，避免写入外层 `loop` 的槽。
fn main() {
    // 1. `loop { break <int> }` → 循环表达式值为 i64
    let a = loop { break 5 };
    println(a);                                  // 5

    // 2. 多臂 `break`（含 `if` 分支）：值经承载槽在出口块汇合
    let mut i = 0;
    let b = loop {
        i = i + 1;
        if i == 3 { break i * 10; }
        if i == 9 { break -1; }
    };
    println(b);                                  // 30
    println(i);                                  // 3

    // 3. `loop { break; }` → 循环表达式值为 ()
    let c = loop { break; };
    let d = c;
    println(1);                                  // 1

    // 4. 嵌套：内层 `break` 只终结内层循环
    let mut outer = 0;
    let e = loop {
        let inner = loop { break 7 };
        outer = inner;
        break outer + 1;
    };
    println(e);                                  // 8

    // 5. 带值 `break` 用于「重试 / 查找」惯用法
    let mut n = 0;
    let found = loop {
        n = n + 1;
        if n * n > 20 { break n; }
    };
    println(found);                              // 5

    // 6. 值为字符串（指针类）与浮点
    let s = loop { break String::from("done") };
    println(s);                                  // done
    let f = loop { break 1.5 };
    println(f);                                  // 1.5

    // 7. `while` / `for` 的 `break` 仍为无值跳出（不影响既有语义）
    let mut k = 0;
    while k < 10 {
        if k == 3 { break; }
        k = k + 1;
    }
    println(k);                                  // 3

    let mut v: Vec<i64> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    let mut sum = 0;
    for x in v {
        if x == 3 { break; }
        sum = sum + x;
    }
    println(sum);                                // 3
}
