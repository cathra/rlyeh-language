// V3-B：chain / enumerate 默认方法（消耗式 self，返回包装迭代器）
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

fn main() -> i64 {
    // chain：Range(0,3) + Range(3,5) → 0,1,2,3,4 → 10
    let c = Range { cur: 0, end: 3 }.chain(Range { cur: 3, end: 5 });
    let mut c = c;
    let mut cs = 0;
    for x in c {
        cs = cs + x;
    }

    // enumerate：Range(0,4) → 序号 0,1,2,3 → 6
    let e = Range { cur: 0, end: 4 }.enumerate();
    let mut e = e;
    let mut es = 0;
    for x in e {
        es = es + x;
    }

    // 链式：filter → enumerate（序号来自过滤后的元素）
    let fe = Range { cur: 0, end: 6 }.filter(|x| x % 2 == 0).enumerate();
    let mut fe = fe;
    let mut fes = 0;
    for x in fe {
        fes = fes + x;
    }

    cs + es + fes
}
