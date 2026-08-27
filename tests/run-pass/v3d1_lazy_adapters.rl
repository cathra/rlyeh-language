// V3-D1：filter/take/skip/collect 惰性默认方法（自定义迭代器）
// filter/take/skip 消耗 self 返回 Filter/Take/Skip 包装迭代器（惰性），
// collect 收集到 Vec<i64>。

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

fn is_even(x: i64) -> bool {
    x % 2 == 0
}

fn is_big(x: i64) -> bool {
    x > 3
}

fn main() -> i64 {
    // filter：0..6 偶数 → 0+2+4 = 6
    let f = Range { cur: 0, end: 6 }.filter(is_even);
    let mut f = f;
    let mut fs = 0;
    for x in f {
        fs = fs + x;
    }

    // take：0..10 前 3 → 0+1+2 = 3
    let t = Range { cur: 0, end: 10 }.take(3);
    let mut t = t;
    let mut ts = 0;
    for x in t {
        ts = ts + x;
    }

    // skip：0..6 跳前 4 → 4+5 = 9
    let sk = Range { cur: 0, end: 6 }.skip(4);
    let mut sk = sk;
    let mut ss = 0;
    for x in sk {
        ss = ss + x;
    }

    // 链式：filter(偶数) → take(2) → 0+2 = 2
    let c = Range { cur: 0, end: 6 }.filter(is_even).take(2);
    let mut c = c;
    let mut cs = 0;
    for x in c {
        cs = cs + x;
    }

    // collect：filter → collect 到 Vec，求和
    let v = Range { cur: 0, end: 6 }.filter(is_even).collect();
    let mut vs = 0;
    for x in v {
        vs = vs + x;
    }

    fs + ts + ss + cs + vs
}
