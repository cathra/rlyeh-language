// V5d+ HashMap 按键集合运算符糖（2026-09-02）：`|`=并集 `&`=交集
// 对标 Python dict 合并：`a | b` 冲突键取 b（右操作数）的值；`a & b` 值取 a（左操作数）。
fn main() {
    // 并集 `|`：冲突键（3）取右操作数 b 的值（300），其余按存在性并入
    let mut a: HashMap<i64, i64> = HashMap::new();
    a.insert(1, 10); a.insert(2, 20); a.insert(3, 30);
    let mut b: HashMap<i64, i64> = HashMap::new();
    b.insert(3, 300); b.insert(4, 40);
    let u = a | b;
    println(u.len());            // 4
    println(u.get(3).unwrap());  // 300
    println(u.get(1).unwrap());  // 10
    println(u.get(4).unwrap());  // 40

    // 交集 `&`：仅共有键（3），值取左操作数 a（30）
    let mut p: HashMap<i64, i64> = HashMap::new();
    p.insert(1, 10); p.insert(2, 20); p.insert(3, 30);
    let mut q: HashMap<i64, i64> = HashMap::new();
    q.insert(3, 300); q.insert(4, 40);
    let i = p & q;
    println(i.len());            // 1
    println(i.get(3).unwrap());  // 30

    // 不相交：交集为空
    let mut x: HashMap<i64, i64> = HashMap::new();
    x.insert(1, 10); x.insert(2, 20);
    let mut y: HashMap<i64, i64> = HashMap::new();
    y.insert(5, 50);
    let di = x & y;
    println(di.len());           // 0

    // 不相交：并集 = 键并集大小
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 10); m.insert(2, 20);
    let mut n: HashMap<i64, i64> = HashMap::new();
    n.insert(5, 50); n.insert(6, 60);
    let un = m | n;
    println(un.len());           // 4
}
