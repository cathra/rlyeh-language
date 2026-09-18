// V3-D3：fold 惰性默认方法（自定义迭代器）
// fold 基于 next() 遍历归约累加（MVP 累加器 i64），impl 未显式实现时走 trait 默认方法。

struct Range {
    cur: i64,
    end: i64,
}

impl Range: Iterator {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.cur < self.end {
            let v = self.cur;
            self.cur = self.cur + 1;
            Option::Some(v)
        } else {
            Option::None
        }
    }
}

fn add(acc: i64, x: i64) -> i64 {
    acc + x
}

fn mul(acc: i64, x: i64) -> i64 {
    acc * x
}

fn sub(acc: i64, x: i64) -> i64 {
    acc - x
}

fn main() -> i64 {
    // fold 累加：1..6 = 15
    let mut r1 = Range { cur: 1, end: 6 };
    let f1 = r1.fold(0, add);

    // fold 累乘：1..5 = 24
    let mut r2 = Range { cur: 1, end: 5 };
    let f2 = r2.fold(1, mul);

    // fold 从 100 减：100-0-1-2-3 = 94
    let mut r3 = Range { cur: 0, end: 4 };
    let f3 = r3.fold(100, sub);

    // 空迭代器 fold：init 原样
    let mut r4 = Range { cur: 5, end: 3 };
    let f4 = r4.fold(7, add);

    f1 + f2 + f3 + f4
}
