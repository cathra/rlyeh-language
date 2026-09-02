// V5b HashSet 集合运算（对标 Python set，2026-09-02）
fn main() {
    let mut a: HashSet<i64> = HashSet::new();
    a.insert(1); a.insert(2); a.insert(3);
    let mut b: HashSet<i64> = HashSet::new();
    b.insert(3); b.insert(4); b.insert(5);

    // 返回新集合：规模确定（槽序不确定，仅校验长度）
    println(a.union(&b).len());              // 5
    println(a.intersection(&b).len());       // 1
    println(a.difference(&b).len());         // 2
    println(a.symmetric_difference(&b).len()); // 4

    // 关系判断
    let mut s1: HashSet<i64> = HashSet::new();
    s1.insert(1);
    let mut s2: HashSet<i64> = HashSet::new();
    s2.insert(1); s2.insert(2);
    println(s1.is_subset(&s2));              // true
    println(s2.is_proper_subset(&s2));        // false
    println(s2.is_superset(&s1));            // true
    println(s1.is_proper_superset(&s1));      // false
    println(s1.is_disjoint(&s2));             // false（共享 1）

    let mut d1: HashSet<i64> = HashSet::new();
    d1.insert(1);
    let mut d2: HashSet<i64> = HashSet::new();
    d2.insert(2);
    println(d1.is_disjoint(&d2));             // true

    // 原地运算
    let mut u: HashSet<i64> = HashSet::new();
    u.insert(1); u.insert(2); u.insert(3);
    u.union_with(&b);
    println(u.len());                         // 5

    u.clear();
    u.insert(1); u.insert(2); u.insert(3);
    u.intersect_with(&b);
    println(u.len());                         // 1

    u.clear();
    u.insert(1); u.insert(2); u.insert(3);
    u.difference_with(&b);
    println(u.len());                         // 2

    u.clear();
    u.insert(1); u.insert(2); u.insert(3);
    u.symmetric_with(&b);
    println(u.len());                         // 4
}
