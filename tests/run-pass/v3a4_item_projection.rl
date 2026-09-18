// V3-A4 验证打印
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
fn main() -> i64 {
    let x: Range::Item = 7;
    let mut r = Range { cur: 10, end: 12 };
    let v: Option<i64> = r.next();
    let mut total = x;
    match v {
        Option::Some(n) => total = total + n,
        Option::None => total = total,
    }
    let mut s = 0;
    let mut r2 = Range { cur: 0, end: 4 };
    for n in r2 {
        s = s + n;
    }
    println(total + s); // 7 + 10 + 6 = 23
    0
}
