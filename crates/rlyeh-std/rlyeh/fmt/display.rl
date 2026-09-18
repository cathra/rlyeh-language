// fmt/display.rl：`Display` protocol —— 2026-09-18 由 fmt/module.rl 拆出。
//
// 归属子模块 `fmt::display`。对外 `fmt::Display` 由 fmt/module.rl 的
// `pub import fmt::display::Display;` 保持（`println!` 等占位符引擎按
// `fmt::Display` / `fmt::Debug` trait 名区分选择 impl，见 check_expr/util.rs）。
//
// X4：`fmt` 返回 `Result<(), FmtError>`（写缓冲 + 错误返回）。

protocol Display {
    fn fmt(&self, f: &mut fmt::formatter::Formatter) -> Result<(), fmt::error::FmtError>;
}
