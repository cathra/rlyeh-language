// benchmark: String push_str 10 万次 —— 与 strcat.zeta 同逻辑
fn main() {
    let mut s = String::new();
    let mut i = 0;
    while i < 100000 {
        s.push_str("ab");
        i = i + 1;
    }
    println!("{}", s.len());
}
