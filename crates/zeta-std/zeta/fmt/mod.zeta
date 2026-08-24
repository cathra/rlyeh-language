// Zeta 标准库格式化模块（阶段 Q3a，2026-08）
//
// 格式化 trait（Display / Debug）+ Formatter 类型，供 `println!` / `print!` /
// `format!` / `dbg!` 的 `{}` / `{:?}` 占位符引擎接入（Q3b，typecheck 特判）。
//
// MVP 签名降级（与 std-lib.md §8 目标 API 的差异）：
// - `Display::fmt` / `Debug::fmt_debug` 直接返回显示字符串（String 拼接模式，
//   与 `serde::Serialize::to_json` 同构；目标 API 为 `Result<(), FmtError>`，
//   需 `()` 返回类型注解 + `FmtError`，MVP 未支持）。
// - `Debug` 方法名用 `fmt_debug` 而非 `fmt`：MVP 方法调用按方法名查找 impl
//   （inherent 优先、trait 次之），Display/Debug 同签名同名方法会歧义。
// - `Formatter` 为约定占位类型（引擎构造 `&mut Formatter::new()` 传入，
//   fmt 体可不使用该参数）；`buf` 字段保留供后续 write_str 引擎扩展。

struct Formatter { buf: String }

impl Formatter {
    pub fn new() -> Formatter { Formatter { buf: String::from("") } }
}

trait Display { fn fmt(&self, f: &mut Formatter) -> String; }

trait Debug { fn fmt_debug(&self, f: &mut Formatter) -> String; }
