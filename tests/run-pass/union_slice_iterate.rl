// 指针元素联合切片 for 迭代：修复 IterRef 步长/槽地址 bug 后，
// `for x in cs.iter()` 中 `x` 应为可用作方法接收者的对象指针（而非元素槽地址）。
fn main() {
    let c: [i64 | String; 3] = [10i64, String::from("ab"), 20i64];
    let cs: &[i64 | String] = &c;
    let mut total = 0;
    for x in cs.iter() {
        match x {
            i64 => total = total + i64,
            String => total = total + (String.len() as i64),
        }
    }
    println(total);            // 10 + 2 + 20 = 32
}
