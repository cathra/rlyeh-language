// X2：标准 TOML 输出 + 解析鲁棒性
// `toml::to_string` 输出标准 `key = value`（`=` 两侧空格）；
// `toml::from_str` 兼容标准空格格式 round-trip。

struct Point { x: i64, y: i64 }
struct Config { name: String, enabled: bool, point: Point }

fn main() -> i64 {
    // 1. 标准空格序列化
    let c = Config { name: String::from("cfg"), enabled: true, point: Point { x: 3, y: 4 } };
    let s = toml::to_string(c);
    // 期望 `name = "cfg"\nenabled = true\npoint = {x = 3,y = 4}`（`=` 两侧空格）

    // 2. round-trip：标准空格格式反序列化
    let c2 = toml::from_str::<Config>(s);
    let mut total = 0;
    if c2.enabled {
        total = total + 1;
    }
    total = total + c2.point.x + c2.point.y; // 1 + 3 + 4 = 8
    total
}
