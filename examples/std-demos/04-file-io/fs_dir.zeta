// N3c：fs 目录操作（create_dir/create_dir_all/remove_file/remove_dir_all/rename/read_dir）
// 使用 /tmp 绝对路径（不依赖工作目录），开头清理保证重复运行幂等。
fn main() {
    // 清理上次残留（幂等）
    let _ = fs::remove_dir_all(String::from("/tmp/zeta_rd"));
    let _ = fs::remove_dir_all(String::from("/tmp/zeta_cda"));
    // create_dir + create_dir_all（多层递归）
    println(fs::create_dir(String::from("/tmp/zeta_rd")).unwrap_or(-1));             // 1
    println(fs::create_dir_all(String::from("/tmp/zeta_cda/x/y/z")).unwrap_or(-1));  // 1
    println(Path::new(String::from("/tmp/zeta_cda/x/y/z")).is_dir());                // 1
    // read_dir（条目列表长度：a.txt + b.txt + 换行 = 12）
    println(fs::write(String::from("/tmp/zeta_rd/a.txt"), String::from("a")).unwrap_or(-1));   // 1
    println(fs::write(String::from("/tmp/zeta_rd/b.txt"), String::from("bb")).unwrap_or(-1));  // 2
    match fs::read_dir(String::from("/tmp/zeta_rd")) {
        Ok(v) => println(v.len),   // 12（"a.txt\nb.txt\n"）
        Err(e) => println(-1),
    }
    // rename + remove_file
    println(fs::rename(String::from("/tmp/zeta_rd/a.txt"), String::from("/tmp/zeta_rd/c.txt")).unwrap_or(-1));  // 1
    println(fs::remove_file(String::from("/tmp/zeta_rd/c.txt")).unwrap_or(-1));     // 1
    // remove_dir_all（清理）
    println(fs::remove_dir_all(String::from("/tmp/zeta_rd")).unwrap_or(-1));        // 1
    println(fs::remove_dir_all(String::from("/tmp/zeta_cda")).unwrap_or(-1));       // 1
}
