// N3b：fs 核心读写（fs::read_to_string / fs::write / fs::copy）
// 使用 /tmp 绝对路径（不依赖工作目录），末尾清理。
fn main() {
    // fs::write + fs::read_to_string
    println(fs::write(String::from("/tmp/zeta_fs.txt"), String::from("fs data")).unwrap_or(-1));   // 7
    match fs::read_to_string(String::from("/tmp/zeta_fs.txt")) {
        Ok(v) => println(v),   // fs data
        Err(e) => println(-1),
    }
    // fs::copy
    println(fs::copy(String::from("/tmp/zeta_fs.txt"), String::from("/tmp/zeta_fs2.txt")).unwrap_or(-1));  // 7
    match fs::read_to_string(String::from("/tmp/zeta_fs2.txt")) {
        Ok(v) => println(v),   // fs data
        Err(e) => println(-1),
    }
    // 清理
    let _ = fs::remove_file(String::from("/tmp/zeta_fs.txt"));
    let _ = fs::remove_file(String::from("/tmp/zeta_fs2.txt"));
}
