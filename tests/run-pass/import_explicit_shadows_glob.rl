// SH-IMPORT-B（2026-09-19）：显式 import 优先于 glob，不产生歧义。
//
// `a` 与 `b` 均导出 `f`；两个 `import ::*` 各引入一次 `f`，但 `import a::f`
// 显式导入后 `f` 落入 `explicit_imports`，glob 歧义判定予以豁免，调用绑定到 `a::f`。

module a { pub fn f() -> i64 { 1 } }
module b { pub fn f() -> i64 { 2 } }

import a::*;
import b::*;
import a::f;

fn main() {
    println(f());   // 1
}
