// Y6b：From/Into 错误转换（std io/error.rl）
// `protocol From<T> { fn from(v: T) -> Self; }` + `impl From<IoErrorKind> for IoError`
// （复用 from_kind 的默认 message 生成）。`From::from` 关联调用 + `into()` 显式转换。

fn main() {
    // 1. From::from 关联调用：IoErrorKind → IoError（默认 message）
    let e = IoError::from(IoErrorKind::NotFound);
    println(e.message()); // entity not found

    // 2. From::from 另一分类
    let e2 = IoError::from(IoErrorKind::PermissionDenied);
    println(e2.message()); // permission denied

    // 3. 显式 into()：IoErrorKind 经 From 转 IoError
    let e3 = IoError::from(IoErrorKind::TimedOut);
    println(e3.message()); // operation timed out
}
