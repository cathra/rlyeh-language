// 模块 + import 导入：编译必须成功
module math {
    pub const PI: f64 = 3.14159;

    pub fn square(x: i64) -> i64 {
        x * x
    }
}

import math::PI;
import math::square as sq;

fn main() {
    println(sq(9));
    println(PI);
}
