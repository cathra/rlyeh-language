// 对应 docs/guide/07-modules.md —— 模块系统（多文件模块）
// 与 main.rl 一起编译：rlyeh run 07-modules/main.rl 07-modules/geometry.rl
module geometry {
    pub struct Point { x: f64, y: f64 }
    pub fn dist(a: Point, b: Point) -> f64 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        dx * dx + dy * dy          // 平方距离（示意，未开方）
    }
    fn secret() -> i64 { 42 }       // 无 pub：模块私有，外部不可 import
}
