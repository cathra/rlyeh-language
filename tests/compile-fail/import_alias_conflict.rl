// expect: conflicts with an existing binding or import
// SH-IMPORT-B（2026-09-19）：同名显式 import 别名冲突（绑定到不同目标）。

module a { pub fn f() -> i64 { 1 } }
module b { pub fn g() -> i64 { 2 } }

import a::f as h;
import b::g as h;

fn main() {
    println(h());
}
