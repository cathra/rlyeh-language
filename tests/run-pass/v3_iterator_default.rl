// V3 Iterator trait 默认方法（2026-08-26）：
// 自定义迭代器实现 std `impl Iterator`（next -> Option<i64>），count/sum/any/all
// 走 trait 默认实现（impl 未显式实现时回退）。

fn is_even(x: i64) -> bool {
    x % 2 == 0
}

fn is_positive(x: i64) -> bool {
    x > 0
}

struct Range {
    cur: i64,
    end: i64,
}

impl Iterator for Range {
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

fn main() {
    // 1. count：遍历计数
    let mut r = Range { cur: 0, end: 5 };
    println(r.count()); // 5
    let mut r0 = Range { cur: 0, end: 0 };
    println(r0.count()); // 0

    // 2. sum：元素求和
    let mut r2 = Range { cur: 1, end: 6 }; // 1+2+3+4+5 = 15
    println(r2.sum()); // 15

    // 3. any：任一满足谓词
    let mut r3 = Range { cur: 1, end: 4 }; // 1,2,3
    println(r3.any(is_even)); // true（2 是偶数）
    let mut r4 = Range { cur: 1, end: 3 }; // 1,2
    println(r4.any(|x| x > 100)); // false

    // 4. all：全部满足谓词
    let mut r5 = Range { cur: 2, end: 6 }; // 2,3,4,5
    println(r5.all(is_positive)); // true
    let mut r6 = Range { cur: -2, end: 2 }; // -2,-1,0,1
    println(r6.all(is_positive)); // false
}
