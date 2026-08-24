// M1a：IoErrorKind C-like 枚举（io 子模块定义，core.zeta re-export）
// M1b：IoError 结构构造 + kind()/message() 访问器
fn kind_code(k: IoErrorKind) -> i64 {
    match k {
        IoErrorKind::NotFound => 1,
        IoErrorKind::PermissionDenied => 2,
        IoErrorKind::AlreadyExists => 3,
        IoErrorKind::InvalidInput => 4,
        IoErrorKind::WouldBlock => 5,
        IoErrorKind::TimedOut => 6,
        IoErrorKind::Other => 7,
    }
}

fn main() {
    // M1a：构造各变体 + match 匹配
    println(kind_code(IoErrorKind::NotFound));
    println(kind_code(IoErrorKind::PermissionDenied));
    println(kind_code(IoErrorKind::AlreadyExists));
    println(kind_code(IoErrorKind::InvalidInput));
    println(kind_code(IoErrorKind::WouldBlock));
    println(kind_code(IoErrorKind::TimedOut));
    println(kind_code(IoErrorKind::Other));

    // M1b：构造 + 访问器
    let e = IoError::new(IoErrorKind::NotFound, String::from("no such file"));
    println(kind_code(e.kind()));   // 1
    println(e.message());           // no such file
    let e2 = IoError::new(IoErrorKind::PermissionDenied, String::from("denied"));
    println(kind_code(e2.kind()));  // 2
    println(e2.message());          // denied
}
