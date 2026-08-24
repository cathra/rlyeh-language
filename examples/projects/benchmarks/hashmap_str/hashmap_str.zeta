// 基准: hashmap_str —— 1 万条字符串键 HashMap 插入与查询
// 测: 字符串哈希 + 键比较 + 运行时键构造（format! 生成 "key_i"）
// 逻辑: 插入 10000 条 "key_i" -> i，再查询 10000 次累加。
//       输出 = Σ(0..9999) = 49,995,000
fn main() {
    let n = 10000;
    let mut m: HashMap<String, i64> = HashMap::new();
    let mut i = 0;
    while i < n {
        let k = format!("key_{}", i);
        m.insert(k, i);
        i = i + 1;
    }
    let mut sum = 0;
    let mut j = 0;
    while j < n {
        let k = format!("key_{}", j);
        match m.get(k) {
            Option::Some(v) => {
                sum = sum + v;
            }
            Option::None => {
                sum = sum - 1;
            }
        }
        j = j + 1;
    }
    println(sum);
}
