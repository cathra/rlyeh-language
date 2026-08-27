// J3 迭代器适配器：map / filter / fold / collect / take / skip
// （数组 / Vec / 自定义迭代器接收者，经 H2 无捕获闭包）
struct Counter {
    limit: i64,
    pos: i64,
}

impl Iterator for Counter {
    type Item = i64;
    fn new(limit: i64) -> Counter {
        Counter { limit: limit, pos: 0 }
    }
    fn next(&mut self) -> Option<i64> {
        if self.pos >= self.limit {
            return Option::None;
        }
        let v = self.pos;
        self.pos += 1;
        Option::Some(v)
    }
}

fn sum_vec(v: Vec<i64>) -> i64 {
    let mut s = 0;
    for x in v {
        s += x;
    }
    s
}

fn main() {
    // 1. 数组 map
    let m = [1, 2, 3].map(|x| x * 2);
    println(sum_vec(m));         // 12

    // 2. 数组 filter（取奇数）
    let f = [1, 2, 3, 4, 5].filter(|x| x % 2 == 1);
    println(sum_vec(f));         // 9

    // 3. 数组 fold
    let t = [1, 2, 3, 4].fold(0, |acc, x| acc + x);
    println(t);                  // 10

    // 4. 数组 collect
    let c = [7, 8, 9].collect();
    println(sum_vec(c));         // 24

    // 5. 数组 take（前 3 个）
    let tk = [1, 2, 3, 4, 5].take(3);
    println(sum_vec(tk));        // 6

    // 6. 数组 skip（跳过 2 个）
    let sk = [1, 2, 3, 4, 5].skip(2);
    println(sum_vec(sk));        // 12

    // 7. 迭代器 map（0..<5 的 *10）→ collect
    let im = Counter::new(5).map(|x| x * 10).collect();
    println(sum_vec(im));        // 100

    // 8. 迭代器 filter（偶数）→ collect 收集到 Vec
    let itf = Counter::new(6).filter(|x| x % 2 == 0).collect();
    println(sum_vec(itf));       // 0+2+4 = 6

    // 9. 迭代器 fold
    let itt = Counter::new(4).fold(100, |acc, x| acc - x);
    println(itt);                // 100-0-1-2-3 = 94

    // 10. 迭代器 take → collect
    let itk = Counter::new(10).take(4).collect();
    println(sum_vec(itk));       // 0+1+2+3 = 6

    // 11. 迭代器 skip → collect
    let its = Counter::new(6).skip(4).collect();
    println(sum_vec(its));       // 4+5 = 9

    // 12. 链式：filter → map → collect（惰性包装迭代器）
    let chained = Counter::new(6).filter(|x| x > 1).map(|x| x * x).collect();
    println(sum_vec(chained));   // 2^2+3^2+4^2+5^2 = 54

    // 13. Vec 上取 skip + collect 组合
    let v = [10, 20, 30, 40].skip(1);
    let w = v.map(|x| x / 10);
    println(sum_vec(w));         // 2+3+4 = 9
}
