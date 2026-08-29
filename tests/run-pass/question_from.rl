// P6c：`?` 运算符错误类型自动 From 转换 + trait 关联函数调用（From::from）。
// inner 返回 `Result<i64, IoErrorKind>`，`?` 经 `From::<IoErrorKind>::from`
// 自动转换为外层 `Result<i64, IoError>` 的错误类型（语义对齐 Rust `E: Into<F>`）。

fn inner() -> Result<i64, io::error::IoErrorKind> {
    Result::Err(io::error::IoErrorKind::NotFound)
}

fn outer() -> Result<i64, io::error::IoError> {
    let v = inner()?;   // IoErrorKind → IoError（From 自动转换）
    Result::Ok(v + 1)
}

fn main() {
    // 1. `?` From 自动转换：内层 Err(IoErrorKind) 转换为外层 IoError
    match outer() {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e.message()),
    }

    // 2. 手动 trait 关联函数调用 From::from（IoErrorKind → IoError）
    let e = From::from(io::error::IoErrorKind::PermissionDenied);
    println(e.message());
}
