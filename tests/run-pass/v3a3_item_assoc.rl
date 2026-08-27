// V3-A3：Iterator::Item 关联类型 + 各 impl 具体化
// 显式提供 type Item = i64; + next -> Option<Self::Item>

struct Range {
    cur: i64,
    end: i64,
}

impl Iterator for Range {
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

fn main() {
    // 经 for 循环接入
    let mut total = 0;
    let mut r = Range { cur: 0, end: 5 };
    for x in r {
        total = total + x;
    }
    println(total); // 0+1+2+3+4 = 10

    // 默认方法
    let mut r2 = Range { cur: 1, end: 4 };
    println(r2.count()); // 3
}
