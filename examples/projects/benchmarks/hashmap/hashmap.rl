// benchmark: HashMap<i64,i64> 20 万次 insert + 20 万次 get（LCG 键）—— 哈希表
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    let mut i = 0;
    let mut x = 12345;
    while i < 200000 {
        x = x * 6364136223846793005 + 1442695040888963407;
        m.insert(x, i);
        i = i + 1;
    }
    let mut sum: i64 = 0;
    let mut j = 0;
    let mut y = 12345;
    while j < 200000 {
        y = y * 6364136223846793005 + 1442695040888963407;
        let g = m.get(y);
        match g {
            Option::Some(v) => { sum = sum + v; }
            Option::None => { sum = sum + 1; }
        }
        j = j + 1;
    }
    println(sum);
}
