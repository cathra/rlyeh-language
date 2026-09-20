// expect: cannot import
// SH-IMPORT-B（2026-09-19）：import 目标模块不存在 → NameNotFound（带候选）。

import nonexist::x;

fn main() {}
