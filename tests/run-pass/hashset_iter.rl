// V5c HashSet 只读引用迭代器（2026-09-02）
fn main() {
    let mut s: HashSet<i64> = HashSet::new();
    s.insert(10);
    s.insert(20);
    s.insert(30);

    // 借用迭代：规模与求和（槽序不确定，仅校验确定性聚合）
    let mut count = 0;
    let mut sum = 0;
    for x in s.iter() {
        count = count + 1;
        sum = sum + *x;
    }
    println(count);   // 3
    println(sum);     // 60

    // 迭代中零拷贝读取：引用解引用比对
    let mut found10 = false;
    for x in s.iter() {
        if *x == 10 {
            found10 = true;
        }
    }
    println(found10);  // true

    // 空集合迭代 0 次
    let mut empty: HashSet<i64> = HashSet::new();
    let mut ec = 0;
    for x in empty.iter() {
        ec = ec + 1;
    }
    println(ec);  // 0
}
