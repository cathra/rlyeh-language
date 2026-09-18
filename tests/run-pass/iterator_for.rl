// J2 自定义迭代器接入 for：`next() -> Option<T>` 方法（inherent impl）
struct Counter {
    limit: i64,
    pos: i64,
}

impl Counter {
    fn new(limit: i64) -> Counter {
        Counter { limit: limit, pos: 0 }
    }
    fn next(&mut self) -> Option<i64> {
        if self.pos >= self.limit {
            return None;
        }
        let v = self.pos;
        self.pos += 1;
        Some(v)
    }
}

// trait 迭代器：`impl NextIter for Step` 同样接入 for
protocol NextIter {
    fn next(&mut self) -> Option<i64>;
}

struct Step {
    cur: i64,
    step: i64,
}

impl Step: NextIter {
    fn next(&mut self) -> Option<i64> {
        let v = self.cur;
        self.cur += self.step;
        if v > 100 {
            None
        } else {
            Some(v)
        }
    }
}

fn main() {
    // 1. inherent 迭代器接入 for：0+1+2+3+4 = 10
    let mut sum = 0;
    for x in Counter::new(5) {
        sum += x;
    }
    println(sum);

    // 2. 手动 next 循环等价验证：0 1 2
    let mut c = Counter::new(3);
    loop {
        match c.next() {
            Some(v) => println(v),
            None => break,
        }
    }

    // 3. for 后迭代器状态（按值绑定，不影响外部副本）
    let c2 = Counter::new(2);
    let mut total = 0;
    for v in c2 {
        total += v;
    }
    println(total); // 0+1 = 1

    // 4. trait 迭代器接入 for：2, 4, 6, ..., 98, 100 之和
    let st = Step { cur: 2, step: 2 };
    let mut s = 0;
    for v in st {
        s += v;
    }
    println(s); // (2+4+...+100) = 50*102/2 = 2550
}
