// fmt/module.rl：格式化模块——只做「子模块声明 + 暴露内容导出」（2026-09-18 重整）。
//
// 组成：
//   fmt/formatter.rl   struct Formatter                        → 子模块 fmt::formatter
//   fmt/error.rl       enum FmtError                           → 子模块 fmt::error
//   fmt/display.rl     protocol Display                        → 子模块 fmt::display
//   fmt/debug.rl       protocol Debug                          → 子模块 fmt::debug
//
// `pub import` 登记 `fmt::Xxx → fmt::<mod>::Xxx` 重导出别名，使既有引用（用户代码
// `impl Display for T`、`println!`/`format!` 占位符引擎、`#[derive(Debug)]` 展开、
// std 内部）无需改动。裸名导出见标准库根 module.rl。
//
// module 声明顺序 = 收集期注册顺序：formatter / error 须先于 display / debug
// （protocol 方法签名引用 Formatter 与 FmtError）。

module formatter;
pub import formatter::Formatter;
module error;
pub import error::FmtError;
module display;
pub import display::Display;
module debug;
pub import debug::Debug;
