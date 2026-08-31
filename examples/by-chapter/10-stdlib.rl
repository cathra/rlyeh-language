// 对应 docs/guide/10-stdlib.md —— 标准库
// 运行：rlyeh run 10-stdlib.rl
fn main() {
    let m: HashMap<String, i64> = HashMap::new();
    m.insert(String::from("a"), 1);

    // get 返回 Option，必须处理"无此键"
    let v = m.get(String::from("a"));
    match v {
        Some(x) => println(x),
        None => println(0),
    };

    // unwrap_or 提供默认值，避免 C 风格 NULL 隐患
    let n = m.get(String::from("missing")).unwrap_or(0);
    println(n);                            // 0
}
