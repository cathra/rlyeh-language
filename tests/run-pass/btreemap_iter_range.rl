// BTreeMap::iter() 借用键值对迭代器 + range() 区间查询（V5+）。
fn main() {
    let mut m: BTreeMap<i64, i64> = BTreeMap::new();
    m.insert(3, 30);
    m.insert(1, 10);
    m.insert(2, 20);
    m.insert(4, 40);
    let mut sum = 0;
    let mut cnt = 0;
    for e in m.iter() {
        sum = sum + *e.key + *e.val;
        cnt = cnt + 1;
    }
    println(sum);           // (1+10)+(2+20)+(3+30)+(4+40) = 110
    println(cnt);           // 4
    let vs = m.range(2, 4); // keys 2,3
    let mut rs = 0;
    for v in vs {
        rs = rs + v;
    }
    println(rs);            // 20 + 30 = 50
}
