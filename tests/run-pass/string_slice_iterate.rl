// 指针元素（String）切片 for 迭代：修复 IterRef 槽地址 bug 后，
// `for x in cs.iter()` 中 `x` 即对象指针，可直接调用 `.len()` 等方法。
fn main() {
    let c: [String; 3] = [String::from("a"), String::from("bb"), String::from("ccc")];
    let cs: &[String] = &c;
    println(cs[0].len());      // 1，索引读取基线
    for x in cs.iter() {
        println(x.len());      // 1 / 2 / 3
    }
}
