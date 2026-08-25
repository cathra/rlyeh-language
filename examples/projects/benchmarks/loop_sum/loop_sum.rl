// benchmark: 1 亿次 i64 循环累加 —— 整数运算 + 分支
fn main() {
    let mut s: i64 = 0;
    let mut i: i64 = 0;
    while i < 100000000 {
        s = s + i;
        i = i + 1;
    }
    println(s);
}
