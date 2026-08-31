// 对应 docs/guide/07-modules.md —— 模块系统（多文件模块）
// 与 geometry.rl 一起编译：rlyeh run 07-modules/main.rl 07-modules/geometry.rl
import geometry::Point;
import geometry::dist as distance;

fn main() {
    let a = Point { x: 0, y: 0 };
    let b = Point { x: 3, y: 4 };
    println(distance(a, b));        // 25
}
