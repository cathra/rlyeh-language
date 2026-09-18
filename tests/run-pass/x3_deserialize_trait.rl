// X3：Deserialize trait + 手写 impl（-> Self 返回）+ 泛型约束基础
// `trait Deserialize { fn from_json(s: String) -> Self; }`
// 依赖 U4 `-> Self` 返回 + 泛型 trait 约束（T: Trait bound）。

protocol Deserialize {
    fn from_json(s: String) -> Self;
}

struct Point { x: i64, y: i64 }

impl Point: Deserialize {
    fn from_json(s: String) -> Point {
        // 简化 JSON 解析 {"x":3,"y":4}
        let mut x: i64 = 0;
        let mut y: i64 = 0;
        let c1 = s.find("x\":");
        if c1 >= 0 {
            let v1 = s.substring(c1 + 3, s.len());
            let c2 = v1.find(",");
            let mut xv = v1;
            if c2 >= 0 {
                xv = v1.substring(0, c2);
            }
            x = string_to_int(xv.trim());
        }
        let c3 = s.find("y\":");
        if c3 >= 0 {
            let v2 = s.substring(c3 + 3, s.len());
            let c4 = v2.find("}");
            let mut yv = v2;
            if c4 >= 0 {
                yv = v2.substring(0, c4);
            }
            y = string_to_int(yv.trim());
        }
        Point { x: x, y: y }
    }
}

fn main() -> i64 {
    // 手写 impl 调用（Deserialize trait 的 from_json -> Self）
    let p = Point::from_json("{\"x\":3,\"y\":4}");
    let p2 = Point::from_json("{\"x\":10,\"y\":20}");
    (p.x + p.y) + (p2.x + p2.y)
}
