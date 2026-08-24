// benchmark: 20 万 insert + 20 万 get（LCG 键）—— 与 hashmap.zeta 同逻辑
use std::collections::HashMap;

fn main() {
    let mut m: HashMap<i64, i64> = HashMap::with_capacity(400000);
    let mut x: i64 = 12345;
    let mut i = 0;
    while i < 200000 {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        m.insert(x, i);
        i = i + 1;
    }
    let mut sum: i64 = 0;
    let mut y: i64 = 12345;
    let mut j = 0;
    while j < 200000 {
        y = y.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        match m.get(&y) {
            Some(v) => sum = sum + v,
            None => sum = sum + 1,
        }
        j = j + 1;
    }
    println!("{}", sum);
}
