// SH-P1-3 B-3（2026-09-02）：glob 导入 `import a::*`。
//
// 一次导入模块内全部直接可见符号（不含嵌套子模块符号），各符号本名注入当前作用域。

module mymod {
    pub const PI: i64 = 3;
    pub fn square(x: i64) -> i64 { x * x }
    pub fn cube(x: i64) -> i64 { x * x * x }
}

import mymod::*;

fn main() {
    println(PI);          // 3
    println(square(3));   // 9
    println(cube(2));     // 8
}
