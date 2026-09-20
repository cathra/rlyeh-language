// fmt/debug.rl：`Debug` protocol —— 2026-09-18 由 fmt/module.rl 拆出。
//
// 归属子模块 `fmt::debug`。对外 `fmt::Debug` 由 fmt/module.rl 的
// `pub import fmt::debug::Debug;` 保持（`#[derive(Debug)]` 生成的 impl 经
// `protocol_name: "fmt::Debug"` 解析，`resolve_protocol_key` 亦支持 `::Debug` 后缀兜底）。
//
// X4：`Debug::fmt_debug` 改名 `Debug::fmt`，与 `Display::fmt` 同名——引擎经
// `find_impl_for_protocol_method`（`fmt::Display` / `fmt::Debug` protocol 名区分）选择。

protocol Debug {
    fn fmt(&self, f: &mut fmt::formatter::Formatter) -> Result<(), fmt::error::FmtError>;
}
