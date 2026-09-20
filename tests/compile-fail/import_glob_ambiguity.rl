// expect: imported via glob from multiple modules
// SH-IMPORT-B（2026-09-19）：两个 glob 导入同名类型，作为类型使用时惰性报歧义。

module a { pub struct T { x: i64 } }
module b { pub struct T { y: i64 } }

import a::*;
import b::*;

fn main() {
    let v: T = a::T { x: 1 };
}
