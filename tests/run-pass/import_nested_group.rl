// SH-P1-3 嵌套组导入 `import a::{b::{x, y}, c}`（2026-09-04 补齐）。
//
// 组内含子组：叶子名 x / y / c 入作用域；子组前缀 `b` 仅作下钻前缀，不在
// 当前作用域注册为可直呼的符号。

module outer {
    module inner {
        pub fn x() -> i64 { 10 }
        pub fn y() -> i64 { 20 }
    }
    pub fn c() -> i64 { 30 }
}

import outer::{inner::{x, y}, c};

fn main() {
    println(x());    // 10
    println(y());    // 20
    println(c());    // 30
}
