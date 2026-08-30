// X2（2026-08-30）：TOML 多行字符串 `"""..."""` 反序列化（单行形式，不含物理换行）。
struct Desc { desc: String }
struct Doc { s: Desc }

fn main() -> i64 {
    // 顶层 `"""..."""` → 剥离 `"""` 定界得原始内容
    let top = toml::from_str::<String>("\"\"\"hello world\"\"\"");
    println(top);
    // `[section]` 内 struct 字段 `desc = """..."""`
    let t = "[s]\ndesc = \"\"\"multi line\"\"\"";
    let d = toml::from_str::<Doc>(t);
    println(d.s.desc);
    0
}
