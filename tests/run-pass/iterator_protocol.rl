// T2 Iterator protocol（MVP 退化：无关联类型 type Item，元素固定 i64）：
// 自定义迭代器 impl Iterator for T 后经 for 循环接入（check_for_iterator
// 检测 next() 方法，inherent 或 protocol impl 均可）
// 计数器迭代器：从 start 递增到 end（含），步长 1
struct Counter {
    cur: i64,
    end: i64,
}
impl Counter: Iterator {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.cur > self.end {
            return Option::None;
        }
        let v = self.cur;
        self.cur = self.cur + 1;
        Option::Some(v)
    }
}

// 步长迭代器：从 start 到 end（不含），步长 step
struct Step {
    cur: i64,
    end: i64,
    step: i64,
}
impl Step: Iterator {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.cur >= self.end {
            return Option::None;
        }
        let v = self.cur;
        self.cur = self.cur + self.step;
        Option::Some(v)
    }
}

fn main() {
    // 1. Counter 经 protocol 接入 for：1+2+3+4 = 10
    let mut c = Counter { cur: 1, end: 4 };
    let mut sum = 0;
    for x in c {
        sum = sum + x;
    }
    println(sum); // 10

    // 2. 空迭代器：cur > end 立即 None
    let mut e = Counter { cur: 5, end: 3 };
    let mut n = 0;
    for x in e {
        n = n + 1;
    }
    println(n); // 0

    // 3. Step 步长 2：0, 2, 4, 6 → 和 12
    let mut st = Step { cur: 0, end: 8, step: 2 };
    let mut s2 = 0;
    for x in st {
        s2 = s2 + x;
    }
    println(s2); // 12

    // 4. 直接调用 next（protocol 方法）：与 for 一致
    let mut c2 = Counter { cur: 1, end: 3 };
    match c2.next() {
        Option::Some(v) => println(v), // 1
        Option::None => println(-1),
    }
    match c2.next() {
        Option::Some(v) => println(v), // 2
        Option::None => println(-1),
    }
    match c2.next() {
        Option::Some(v) => println(v), // 3
        Option::None => println(-1),
    }
    match c2.next() {
        Option::Some(v) => println(v),
        Option::None => println(-1), // -1（耗尽）
    }
}
