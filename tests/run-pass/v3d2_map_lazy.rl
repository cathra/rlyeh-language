// V3-D2：map 惰性默认方法（返回 Map 包装迭代器）
// map 消耗 self 返回 Map<Self, i64>（MVP 变换返回 i64），next() 变换元素。

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

fn dbl(x: i64) -> i64 {
    x * 2
}

fn sq(x: i64) -> i64 {
    x * x
}

fn main() -> i64 {
    // map：0..3 *2 → 0+2+4 = 6
    let m = Range { cur: 0, end: 3 }.map(dbl);
    let mut m = m;
    let mut ms = 0;
    for x in m {
        ms = ms + x;
    }

    // map → collect：0..5 *2 → 0,2,4,6,8
    let v = Range { cur: 0, end: 5 }.map(dbl).collect();
    let mut vs = 0;
    for x in v {
        vs = vs + x;
    }

    // filter → map → collect：0..6 偶数 *自身 → 0,4,16 → 20
    let c = Range { cur: 0, end: 6 }
        .filter(|x| x % 2 == 0)
        .map(sq)
        .collect();
    let mut cs = 0;
    for x in c {
        cs = cs + x;
    }

    ms + vs + cs
}
