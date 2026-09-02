// V5d+ 集合关系运算符糖（2026-09-02）：< <= > >= 经 PartialOrd 重载为子集/超集语义
//（与 Python `set` 对齐：`<`=真子集 `<=`=子集 `>`=真超集 `>=`=超集）
fn main() {
    let mut a: HashSet<i64> = HashSet::new();
    a.insert(1); a.insert(2); a.insert(3);                 // A = {1,2,3}
    let mut b: HashSet<i64> = HashSet::new();
    b.insert(1); b.insert(2); b.insert(3); b.insert(4);    // B = A ∪ {4}
    let mut c: HashSet<i64> = HashSet::new();
    c.insert(5);                                           // C = {5}（与 A 不相交）
    let mut d: HashSet<i64> = HashSet::new();
    d.insert(1); d.insert(2); d.insert(3); d.insert(4); d.insert(5);  // D = B ∪ {5}

    // 真子集：A ⊂ B
    println(a < b);     // true
    println(a <= b);    // true
    // 相等集合：A 不真包含于自身，但包含于自身
    println(a < a);     // false
    println(a <= a);    // true
    // 真超集：B ⊃ A
    println(b > a);     // true
    println(b >= a);    // true
    // 不相交：C 与 A 互不包含
    println(a < c);     // false
    println(c > a);     // false
    // 正向比较链 a < b < d（链式复用操作数 b，须无二次 move）
    println(a < b < d); // true  (A ⊂ B ⊂ D)
    // 反向比较链 d > b > a：(D ⊃ B) || (B ⊃ A)
    println(d > b > a); // true
}
