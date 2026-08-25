// N3a：Path 对象（new/join/parent/file_name/extension/exists/is_file/is_dir）
// exists/is_file/is_dir 使用 /tmp 绝对路径（不依赖工作目录），自创建自清理。
fn main() {
    // join（自动处理尾部斜杠）
    let p1 = Path::new(String::from("a/b"));
    println(p1.join(String::from("c.txt")).as_string());       // a/b/c.txt
    let p2 = Path::new(String::from("a/b/"));
    println(p2.join(String::from("c")).as_string());           // a/b/c
    // parent
    println(Path::new(String::from("a/b/c.txt")).parent().as_string());   // a/b
    println(Path::new(String::from("c.txt")).parent().as_string());       // .
    // file_name
    println(Path::new(String::from("a/b/c.txt")).file_name().as_string()); // c.txt
    // extension
    println(Path::new(String::from("a/b/c.txt")).extension());  // txt
    println(Path::new(String::from("noext")).extension());      // 空串
    // exists / is_file / is_dir（自创建文件 + 系统目录）
    let _ = fs::remove_file(String::from("/tmp/rlyeh_path_ops.txt"));
    let _ = fs::write(String::from("/tmp/rlyeh_path_ops.txt"), String::from("x"));
    println(Path::new(String::from("/tmp/rlyeh_path_ops.txt")).exists());    // 1
    println(Path::new(String::from("/tmp/rlyeh_path_ops.txt")).is_file());   // 1
    println(Path::new(String::from("/tmp")).is_dir());                      // 1
    println(Path::new(String::from("/tmp/rlyeh_no_such_path_zz.txt")).exists());  // 0
    let _ = fs::remove_file(String::from("/tmp/rlyeh_path_ops.txt"));
}
