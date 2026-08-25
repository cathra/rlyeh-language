// ===== 定点数运算（i64 定点，SCALE = 1000）=====
//
// MVP 无 f64↔i64 转换与数学库（sqrt/tan/pow 均未内置），
// 本模块以纯整数模拟定点浮点运算，供 raytracer 全程使用：
//   - 1 个"单位" = 1000（SCALE），如 3.5 表示为 3500；
//   - 乘法/除法/平方根/幂/夹取均在此层保证量纲一致。
// 这也展示了 Zeta 纯整数运算与模块化组织能力。

pub const SCALE: i64 = 1000;

// 定点乘法：a * b / SCALE
pub fn fmul(a: i64, b: i64) -> i64 {
    (a * b) / SCALE
}

// 定点除法：a / b（b 为定点表示）
pub fn fdiv(a: i64, b: i64) -> i64 {
    (a * SCALE) / b
}

// 定点平方根（牛顿迭代；n 为"定点平方"量纲，返回定点量纲）
pub fn fsqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n / 1000000;
    if x < 1 {
        x = 1;
    }
    let mut iter = 0;
    while iter < 32 {
        let q = n / x;
        let s = x + q;
        x = s / 2;
        iter = iter + 1;
    }
    x
}

// 幂：定点底数 b，整数指数 e，返回 b^e（×1000）
pub fn fpow(b: i64, e: i64) -> i64 {
    let mut acc = SCALE;
    let mut i = 0;
    while i < e {
        acc = fmul(acc, b);
        i = i + 1;
    }
    acc
}

// 夹取到 [lo, hi]
pub fn clamp(v: i64, lo: i64, hi: i64) -> i64 {
    if v < lo {
        return lo;
    }
    if v > hi {
        return hi;
    }
    v
}

// 非负（漫反射系数等）
pub fn max0(v: i64) -> i64 {
    if v < 0 {
        return 0;
    }
    v
}
