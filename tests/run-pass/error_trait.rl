// M2a：Error trait + impl Error for IoError + dyn 调用（H4）
// M2b：错误转换约定 IoError::from_kind（From/Into 泛型 trait 声明已支持，
//       blanket impl 受 where 约束限制，MVP 用窄化转换入口）
// dyn 仅作局部变量（H4 MVP 限制：不可作函数参数），describe 内部构造局部 dyn 绑定。
fn describe(err: &IoError) -> String {
    let d: dyn Error = err;
    d.message()
}

fn main() {
    // M2a：dyn vtable 分派 + 固有方法共存
    let e = IoError::new(IoErrorKind::NotFound, String::from("missing file"));
    println(describe(&e));   // missing file
    let e2 = IoError::new(IoErrorKind::TimedOut, String::from("timeout"));
    println(describe(&e2));  // timeout
    println(e.message());    // 固有方法（与 trait 方法同名共存）

    // M2b：from_kind 自动生成 message（转换约定）
    let f = IoError::from_kind(IoErrorKind::NotFound);
    println(f.message());         // entity not found
    println(describe(&f));        // dyn 经 Error impl 读取同 message
    let f2 = IoError::from_kind(IoErrorKind::PermissionDenied);
    println(f2.message());        // permission denied
    let f3 = IoError::from_kind(IoErrorKind::WouldBlock);
    println(f3.message());        // operation would block
}
