struct Range { cur: i64, end: i64 }
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
fn is_even(x: i64) -> bool { x % 2 == 0 }
fn main() -> i64 {
    let f = Filter<Range> { inner: Range { cur: 0, end: 6 }, pred: is_even };
    let mut f = f;
    let mut fs = 0;
    for x in f { fs = fs + x; }
    let t = Take<Range> { inner: Range { cur: 0, end: 10 }, remaining: 3 };
    let mut t = t;
    let mut ts = 0;
    for x in t { ts = ts + x; }
    let sk = Skip<Range> { inner: Range { cur: 0, end: 6 }, to_skip: 4 };
    let mut sk = sk;
    let mut ss = 0;
    for x in sk { ss = ss + x; }
    let c = Chain<Range, Range> { a: Range { cur: 0, end: 3 }, b: Range { cur: 3, end: 5 }, on_a: true };
    let mut c = c;
    let mut cs = 0;
    for x in c { cs = cs + x; }
    let e = Enumerate<Range> { inner: Range { cur: 0, end: 4 }, idx: 0 };
    let mut e = e;
    let mut es = 0;
    for x in e { es = es + x; }
    println(fs); // 6
    println(ts); // 3
    println(ss); // 9
    println(cs); // 10
    println(es); // 6
    0
}
