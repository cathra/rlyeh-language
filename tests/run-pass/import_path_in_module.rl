// 2026-09-18：模块内 `import` 的路径解析规则（typecheck `resolve_import_path`）。
//
// - 引用**本模块的子模块**：写相对路径即可（`pub import sub::value;`
//   归一为 `pkg::sub::value`）；
// - 引用**外部 / 顶层模块**：写完整路径（`pub import helper::bump;` 中的 `helper`
//   是顶层模块，相对目标 `pkg::helper::bump` 不存在，故按绝对路径解析）；
// - 模块**外部**（顶层）导入：写完整路径（`import pkg::value;`）。

module helper {
    pub fn bump(x: i64) -> i64 { x + 1 }
}

module pkg {
    module sub {
        pub fn value() -> i64 { 7 }
    }
    // 相对本模块子模块
    pub import sub::value;
    // 跨模块：完整路径（本例为顶层 `helper`）
    pub import helper::bump;
}

// 经 `pkg` 的重导出链访问（`pkg::value` → `pkg::sub::value`；
// `pkg::bump` → `helper::bump`）
import pkg::value;
import pkg::bump;

fn main() {
    println(value());        // 7
    println(bump(41));       // 42
}
