// X2：引号感知解析——值含逗号的字符串在内联表/数组/嵌套容器中正确分段
// `split_quoted`（core.rl）：跳过双引号字符串内的分隔符。

struct Inner { name: String, val: i64 }
struct Outer { inner: Inner }

fn main() -> i64 {
    // 1. 内联表字符串值含逗号
    let o = toml::from_str::<Outer>("inner = {name = \"a,b\", val = 5}");
    let mut total = o.inner.val; // 5

    // 2. 数组元素含逗号字符串
    let v = toml::from_str::<Vec<String>>("[\"x,y\", \"z\"]");
    total = total + v.len(); // 5 + 2 = 7

    total
}
