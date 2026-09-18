// fmt/error.rl：`FmtError`（格式化错误）——2026-09-18 由 fmt/module.rl 拆出。
//
// 归属子模块 `fmt::error`。对外 `fmt::FmtError` 由 fmt/module.rl 的
// `pub import fmt::error::FmtError;` 保持（derive 展开生成的
// `AstType::Path("fmt::FmtError")` 依赖该别名）。
//
// X4：`fmt -> Result<(), FmtError>` 的错误返回载体。

enum FmtError {
    Invalid,
    Fmt(String),
}
