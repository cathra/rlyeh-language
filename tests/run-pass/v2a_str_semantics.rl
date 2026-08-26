// V2-A 审计：&str（StrFat）语义验证
// 1. as_str() 返回 &str 绑定变量后，len/索引/打印
// 2. &str 作为函数参数传递
// 3. 子区间视图 as_str_range
fn main() {
    // 1. as_str 绑定 &str
    let s = String::from("hello");
    let v = s.as_str();
    println(v.len()); // 5
    println(v[1]); // 101（'e'）

    // 2. &str 参数传递
    let n = str_len(v);
    println(n); // 5

    // 3. 子区间视图
    let r = s.as_str_range(1, 3);
    println(r.len()); // 2
}

fn str_len(x: &str) -> i64 {
    x.len()
}
