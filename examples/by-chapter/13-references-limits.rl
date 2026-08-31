// 对应 docs/guide/13-references-limits.md —— 参考与已知限制
// 演示 MVP 已支持的特性集合（对照 §13.3 实现状态表）
// 运行：rlyeh run 13-references-limits.rl
fn main() {
    let arr = [1, 2, 3];                  // 数组
    let v = vec![4, 5, 6];                // Vec 字面量
    println(arr[0]);                      // 1
    println(v[0]);                        // 4

    let s = String::from("hi");
    println(s.len());                     // 2

    let m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("k"), 9);
    println(m.contains_key(String::from("k")));  // true
}
