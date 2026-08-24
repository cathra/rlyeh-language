// 基准: hashmap_str —— 1 万条字符串键哈希表插入与查询
// 与 hashmap_str.zeta 逻辑严格一致。输出 = 49995000
use std::collections::HashMap;

fn main() {
    let n: i64 = 10000;
    let mut m: HashMap<String, i64> = HashMap::with_capacity((n * 2) as usize);
    for i in 0..n {
        m.insert(format!("key_{}", i), i);
    }
    let mut sum: i64 = 0;
    for i in 0..n {
        match m.get(&format!("key_{}", i)) {
            Some(&v) => sum += v,
            None => sum -= 1,
        }
    }
    println!("{}", sum);
}
