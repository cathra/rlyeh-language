// N1a：OpenMode 枚举（io.zeta）+ fopen 模式字符串映射
fn match_code(m: OpenMode) -> i64 {
    match m {
        OpenMode::Read => 1,
        OpenMode::Write => 2,
        OpenMode::Append => 3,
        OpenMode::ReadWrite => 4,
        OpenMode::Create => 5,
    }
}

fn main() {
    // 变体构造 + match（裸名 re-export 与完整路径均可）
    println(match_code(OpenMode::Read));                  // 1
    println(match_code(io::OpenMode::Write));             // 2
    // 模式字符串映射（module 内 helper，跨模块完整路径调用）
    println(io::open_mode_str(io::OpenMode::Read));       // r
    println(io::open_mode_str(io::OpenMode::Write));      // w
    println(io::open_mode_str(io::OpenMode::Append));     // a
    println(io::open_mode_str(io::OpenMode::ReadWrite));  // r+
    println(io::open_mode_str(io::OpenMode::Create));     // w
}
