// 期望编译失败：read_file 已 Result 化（M3a），不能直接注解为 String
// expect: found `Result<String, io::error::IoError>`
fn main() {
    let content: String = read_file(String::from("data.txt"));
    println(content);
}
