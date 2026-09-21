// EH-5（0.2.0-AA，2026-09-21）：`#[derive(Error)]`（thiserror 式）——RFC error-handling
// §4.5 落地验收。
//
// 覆盖：
//  1. struct：项级 `#[error("模板")]` → `message()`（含 `{field}` 插值：String / 整型）；
//  2. struct：`#[source]` 字段 → `source()` 委托；`#[from]` 单字段 → `impl X: From<T>`
//     与 `?` 自动转换；
//  3. enum：逐变体 `#[error("..")]` → `message()` 按变体 `match`（`{0}` 插值覆盖
//     String / i64 / f64 / bool 与单元变体）；
//  4. enum：单个元组字段的 `#[from]` 变体 → `impl X: From<T>` 与 `?` 转换；
//  5. 与 `Error` protocol / `&dyn Error` 上转配合。
//
// 已知限制（本轮实测）：
//  - `Enum::from(x)` 直接静态调用不可用（枚举静态 / 固有方法调用为既有缺口，见
//    `docs/std-lib.md` §12）；`#[from]` 经 `?` 运算符与 `impl From` 注册生效。
//  - `#[source]` 仅 struct 支持（enum 变体级 `#[source]` 待办，`source()` 恒 `None`）。
//  - 同一函数内同名绑定不得跨类型复用（`docs/std-lib.md` §12），故各用例变量名唯一。

#[derive(Error)]
#[error("config: {key} = {value}")]
struct ConfigError {
    key: String,
    value: i64,
}

#[derive(Error)]
#[error("io failure")]
struct WrappedIo {
    #[from]
    #[source]
    inner: IoError,
}

#[derive(Error)]
#[error("load failed")]
enum LoadError {
    #[from]
    #[error("io: {0}")]
    Io(String),
    #[error("not found: {0}")]
    NotFound(i64),
    #[error("timeout after {0} ms")]
    Timeout(f64),
    #[error("enabled: {0}")]
    Flag(bool),
    #[error("bare")]
    Bare,
}

fn io_fail() -> Result<i64, IoError> {
    Result::Err(IoError::from_kind(IoErrorKind::NotFound))
}

fn str_fail() -> Result<i64, String> {
    Result::Err(String::from("boom"))
}

fn wrap_io() -> Result<i64, WrappedIo> {
    let v = io_fail()?;
    Result::Ok(v)
}

fn load() -> Result<i64, LoadError> {
    let v = str_fail()?;
    Result::Ok(v)
}

fn main() {
    // 1. struct：模板插值（String + 整型）
    let cfg = ConfigError { key: String::from("port"), value: 8080 };
    println(cfg.message());                            // config: port = 8080
    println(cfg.source().is_none());                   // 1
    // 2. struct：`#[from]` + `#[source]`
    let wio = WrappedIo::from(IoError::from_kind(IoErrorKind::NotFound));
    println(wio.message());                            // io failure
    println(wio.source().is_none());                   // 1
    println(wrap_io().is_err());                       // 1（`?` 经 `#[from]` 转换）
    // 3. enum：逐变体模板
    let en1 = LoadError::NotFound(7);
    println(en1.message());                            // not found: 7
    let en2 = LoadError::Timeout(1.5);
    println(en2.message());                            // timeout after 1.500000 ms
    let en3 = LoadError::Flag(true);
    println(en3.message());                            // enabled: true
    let en4 = LoadError::Bare;
    println(en4.message());                            // bare
    // 4. enum：`#[from]` + `?`（String → LoadError::Io）
    println(load().is_err());                          // 1
    match load() {
        Result::Ok(_) => println(-1),
        Result::Err(err) => println(err.message()),    // io: boom
    }
    // 5. `Error` protocol 上转后虚调用
    let en5 = LoadError::NotFound(9);
    let dyn_err: &dyn Error = &en5;
    println(dyn_err.message());                        // not found: 9
}
