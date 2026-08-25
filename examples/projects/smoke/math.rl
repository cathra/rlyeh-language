// smoke 模块：数值 + 结构体 + f64
pub fn square(x: i64) -> i64 {
    x * x
}

pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn len2(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }
}
