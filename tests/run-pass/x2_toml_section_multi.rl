// X2：多级 [section] 路径（[a.b] 点号路径，2026-08-27）
// 序列化：深层嵌套 struct 输出 `[inner]\n[inner.p]`（多级路径）；
// 反序列化：解析 `[a.b]` 路径 + 递归归入对应嵌套 struct 字段。

struct Point { x: i64, y: i64 }
struct Inner { p: Point }
struct Outer { tag: i64, inner: Inner }

fn main() -> i64 {
    // 1. 序列化：[inner] + [inner.p] 多级路径
    let c = Outer { tag: 5, inner: Inner { p: Point { x: 7, y: 9 } } };
    let s = toml::to_string(c);
    // 期望 `tag = 5\n[inner]\n[inner.p]\nx = 7\ny = 9`

    // 2. round-trip：[inner.p] 路径归组
    let c2 = toml::from_str::<Outer>(s);
    let mut total = c2.inner.p.x + c2.inner.p.y; // 16

    // 3. 直接 [a.b] 路径输入
    let c3 = toml::from_str::<Outer>("tag = 1\n[inner.p]\nx = 3\ny = 4");
    total = total + c3.inner.p.x + c3.inner.p.y; // + 7 = 23

    total
}
