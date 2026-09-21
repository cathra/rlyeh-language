// EH-4（0.2.0-AA，2026-09-21）：拥有型动态错误 `DynError`——RFC error-handling §4.4
// 「anyhow 式便捷」落地验收。
//
// 覆盖：
//  1. 构造：`IoError::into_dyn()` / `DynError::from_io()`——装箱 → 取堆对象引用 →
//     `dyn Error` 上转 → 再装箱（`Box<dyn Error>` = 指向 2 槽胖指针的对象指针）；
//  2. 转发：`DynError::message()` / `source()` 经内层 `Box<dyn Error>` **虚调用**分派；
//  3. `Error` protocol：`DynError` 自身实现 `Error`，可上转为 `&dyn Error`；
//  4. 与 `Result<T, DynError>` 类型别名 / `?` 传播 / `match` 解构配合；
//  5. 用户侧便捷宏 `bail!` / `ensure!`（M3 落地形态——std 无法导出宏，见
//     `rlyeh-std/rlyeh/io/error.rl` 的宏系统缺口注记）。

type LoadResult<T> = Result<T, DynError>;

// 用户侧宏（M3）：错误表达式用 `$($t:tt)*`（`$e:expr` 不匹配含 `::` 的路径调用）；
// transcriber 为单个表达式（`return ...` 不带结尾分号）；条件显式加括号。
macro_rules! bail {
    ($($t:tt)*) => { return Result::Err($($t)*) };
}

macro_rules! ensure {
    ($cond:expr, $($t:tt)*) => {
        if !($cond) { return Result::Err($($t)*) }
    };
}

fn parse_pos(name: String) -> LoadResult<i64> {
    ensure!(!name.is_empty(), DynError::from_io(IoError::from_kind(IoErrorKind::InvalidInput)));
    if name == "missing" {
        bail!(DynError::from_io(IoError::from_kind(IoErrorKind::NotFound)));
    }
    Result::Ok(name.len())
}

// `?` 在 `Result<_, DynError>` 上下文传播（错误类型相同，无需 From 转换）。
fn forward(name: String) -> LoadResult<i64> {
    let n = parse_pos(name)?;
    Result::Ok(n * 2)
}

fn main() {
    // 1. 构造 + 2. `message` 转发（虚调用 → IoError::message）
    let de: DynError = IoError::new(IoErrorKind::Other, String::from("boom")).into_dyn();
    println(de.message());                                 // boom
    // 3. `source` 转发（IoError::source → None）+ 自身可作 `Error` 上转为 `&dyn Error`
    println(de.source().is_none());                        // 1
    let as_err: &dyn Error = &de;
    println(as_err.message());                             // boom
    // 4. `Result<T, DynError>`：成功 / `ensure!` 失败 / `bail!` 失败
    println(parse_pos(String::from("abcd")).unwrap());     // 4
    println(parse_pos(String::from("")).is_err());         // 1
    match parse_pos(String::from("missing")) {
        Result::Ok(_) => println(-1),
        Result::Err(e) => println(e.message()),            // entity not found
    }
    // 5. `?` 传播
    println(forward(String::from("xy")).unwrap());         // 4
    println(forward(String::from("missing")).is_err());    // 1
}
