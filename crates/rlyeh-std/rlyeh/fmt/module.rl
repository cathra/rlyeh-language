// Rlyeh 标准库格式化模块（阶段 Q3a / X4，2026-08）
//
// 格式化 trait（Display / Debug）+ Formatter 类型，供 `println!` / `print!` /
// `format!` / `dbg!` 的 `{}` / `{:?}` 占位符引擎接入（Q3b，typecheck 特判）。
//
// X4 完整化（2026-08-27）：`fmt` 返回 `Result<(), FmtError>`（写缓冲 + 错误返回）；
// `Debug::fmt_debug` 改名 `Debug::fmt`（同名冲突经 impl 查找按 trait 区分消除，
// typecheck `find_impl_for_trait_method`）；`Formatter` 升级为真实格式化器（持
// 输出缓冲 + 对齐/宽度/精度/填充状态字段 + `write_str`/`result` 访问器）。
//
// MVP 说明：
// - `Result<(), FmtError>` 返回（X4 ✅）：`Result::Ok(())` / `Result::Err(...)` 可构造，
//   手写 `impl Display/Debug` 返回 `Result::Ok(())`（写入经 `f.write_str`）。
// - `Debug::fmt` 与 `Display::fmt` 同名：引擎经 `find_impl_for_trait_method`
//   （`fmt::Display` / `fmt::Debug` trait 名区分）选择，MVP 无 trait bound 检查。
// - Formatter 对齐/宽度/精度/填充字段为状态存储（`fill`/`width`/`align`），
//   对齐格式占位符（`{:>10}`）的完整引擎应用留待后续；`write_str` 直接追加。

struct Formatter {
    buf: String,
    fill: String,
    width: i64,
    align: i64,
}

impl Formatter {
    pub fn new() -> Formatter {
        Formatter {
            buf: String::from(""),
            fill: String::from(" "),
            width: 0,
            align: 0,
        }
    }
    // 写片段到输出缓冲（X4：fmt 体经此累积显示字符串）
    pub fn write_str(&mut self, s: String) {
        self.buf = self.buf + s;
    }
    // 取回拼接结果（引擎/用户经此读取最终显示字符串）
    pub fn result(&self) -> String {
        self.buf
    }
}

// X4：格式化错误类型（`fmt -> Result<(), FmtError>` 的错误返回）。
enum FmtError {
    Invalid,
    Fmt(String),
}

// X4：`fmt` 返回 `Result<(), FmtError>`（写缓冲 + 错误返回）。
trait Display {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

// X4：`Debug::fmt_debug` 改名 `Debug::fmt`（与 Display::fmt 同名，经 impl 查找
// 按 trait 区分）。
trait Debug {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}
