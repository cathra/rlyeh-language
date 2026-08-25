// N2b：stdin 增强（read_to_string / lines 迭代）——空 stdin（测试框架无输入注入）
fn main() {
    // read_to_string：EOF 立即返回 Ok("")
    match read_to_string() {
        Ok(v) => println(v.len),      // 0
        Err(e) => println(-1),
    }
    // lines()：空输入迭代 0 次（J2 for 循环接入）
    let mut count = 0;
    for line in lines() {
        count = count + 1;
    }
    println(count);                   // 0
    // read_line：EOF 返回 Ok("")
    match read_line() {
        Ok(v) => println(v.len),      // 0
        Err(e) => println(-1),
    }
}
