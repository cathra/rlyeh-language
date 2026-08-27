// X2：TOML 注释支持——from_str 跳过纯注释/空行（`#` 开头的整行）
// 行尾注释（`x = 1 # c`）MVP 限制（值含 `#` 文本）见 x2 叶子。

struct A { x: i64, y: i64 }

fn main() -> i64 {
    // 含纯注释行 + 空行的 TOML
    let a = toml::from_str::<A>("# 这是注释\nx = 1\n\n# 另一行\ny = 2");
    a.x + a.y
}
