// X2：[section] 行式子表（完整实现，2026-08-27）
// 序列化：顶层嵌套 struct 输出 `[point]\nx = 7\ny = 9`（标准 TOML 行式）；
// 反序列化：解析 `[section]` 头 + 维护当前 section + 字段按 section 归入嵌套 struct。
// 多级嵌套（`[a.b]` section 路径）为已知限制（见 x2 叶子）。

struct Point { x: i64, y: i64 }
struct Config { name: String, enabled: bool, point: Point }

fn main() -> i64 {
    // 1. 序列化：[section] 行式输出
    let c = Config { name: String::from("cfg"), enabled: true, point: Point { x: 3, y: 4 } };
    let s = toml::to_string(c);
    // 期望 `name = "cfg"\nenabled = true\n[point]\nx = 3\ny = 4`

    // 2. round-trip：[section] 解析归组
    let c2 = toml::from_str::<Config>(s);
    let mut total = 0;
    if c2.enabled {
        total = total + 1;
    }
    total = total + c2.point.x + c2.point.y; // 1 + 3 + 4 = 8

    // 3. 多个嵌套 struct section（顺序无关）
    let c3 = toml::from_str::<Config>("enabled = true\n[point]\nx = 10\ny = 20\nname = \"q\"");
    total = total + c3.point.x + c3.point.y; // + 30 = 38

    total
}
