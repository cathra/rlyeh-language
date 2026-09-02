// SH-P1-3 B-2（2026-09-02）：`pub use` 重导出，外部模块经 `import outer::revealed`
// 访问内层符号。
//
// 链路：`r` → `outer::revealed`（重导出）→ `inner::secret`（真实符号）。
// 验证 `resolve_full_name` 的别名链传递追踪与模块前缀重导出登记。

module inner {
    pub fn secret(x: i64) -> i64 { x + 1 }
}

module outer {
    pub import inner::secret as revealed;
}

import outer::revealed as r;

fn main() {
    println(r(41));       // 42
}
