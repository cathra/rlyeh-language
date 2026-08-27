struct Range { cur: i64, end: i64 }
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
fn is_even(x: i64) -> bool { x % 2 == 0 }
fn is_big(x: i64) -> bool { x > 100 }
fn main() -> i64 {
    let mut r1 = Range { cur: 1, end: 7 };
    let f1 = r1.find(is_even);
    let mut r2 = Range { cur: 1, end: 3 };
    let f2 = r2.find(is_big);
    println(f1); // 2
    println(f2); // -1
    0
}
