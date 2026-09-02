// SH-P1-3 B-1（2026-09-02）：组导入 `import a::{b, c as d}`。
//
// 单条导入语句一次性引入模块内多个符号，并支持成员级 `as` 重命名。

module mymod {
    pub const PI: i64 = 3;
    pub fn square(x: i64) -> i64 { x * x }
}

import mymod::{PI, square as sq};

fn main() {
    println(PI);          // 3
    println(sq(3));       // 9
}
