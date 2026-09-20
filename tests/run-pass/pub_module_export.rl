// SH-IMPORT-A（2026-09-19）：`pub module` 声明解析与编译。
//
// `pub module` 让模块本身可对外暴露（与 `pub import` 重导出对齐）。扁平名字空间下
// 模块已可经 `name::item` 访问，本用例验证 `pub module` 语法被正确解析且程序可
// 编译运行；实际可见性收紧由工作流 C（`--visibility=error`）完成。

pub module inner {
    pub fn secret() -> i64 { 42 }
}

fn main() {
    println(inner::secret());   // 42
}
