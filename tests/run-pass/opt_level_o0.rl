// 优化级别 -O0：MIR pass 关闭、clang -O0，语义必须与 -O3 一致。
// flag: -O0
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    let mut i = 0;
    while i < 100 {
        m.insert(i, i * 3);
        i = i + 1;
    }
    let mut s = 0;
    i = 0;
    while i < 100 {
        match m.get(i) {
            Option::Some(v) => {
                s = s + v;
            }
            Option::None => {
                s = s - 1;
            }
        }
        i = i + 1;
    }
    println(s);
}
