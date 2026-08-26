// V5 新集合：HashSet<T> 哈希集合 + BTreeMap<K, V> 有序映射（2026-08-26）
fn main() {
    // ===== HashSet<T> =====
    // 1. insert / contains / len
    let mut s: HashSet<i64> = HashSet::new();
    s.insert(10);
    s.insert(20);
    s.insert(30);
    println(s.contains(10)); // true
    println(s.contains(99)); // false
    println(s.len()); // 3

    // 2. 重复插入被忽略（集合语义）
    s.insert(10);
    println(s.len()); // 3

    // 3. remove
    let ok = s.remove(20);
    println(ok); // true
    println(s.contains(20)); // false
    println(s.len()); // 2

    // 4. 扩容后元素仍可查（负载 7/8 触发 grow）
    let mut s2: HashSet<i64> = HashSet::with_capacity(2);
    let mut i = 0;
    while i < 20 {
        s2.insert(i * 7);
        i = i + 1;
    }
    println(s2.contains(0)); // true
    println(s2.contains(133)); // true
    println(s2.len()); // 20
    println(s2.contains(500)); // false

    // 5. is_empty / clear
    println(s2.is_empty()); // false
    s2.clear();
    println(s2.len()); // 0
    println(s2.is_empty()); // true

    // ===== BTreeMap<K, V> =====
    // 1. insert / get / len（键升序存储）
    let mut m: BTreeMap<i64, i64> = BTreeMap::new();
    m.insert(3, 300);
    m.insert(1, 100);
    m.insert(2, 200);
    match m.get(2) {
        Option::Some(v) => println(v), // 200
        Option::None => println(-1),
    }
    match m.get(9) {
        Option::Some(v) => println(v),
        Option::None => println(-1), // -1
    }
    println(m.len()); // 3

    // 2. 键保持有序：first / last
    match m.first() {
        Option::Some(k) => println(k), // 1
        Option::None => println(-1),
    }
    match m.last() {
        Option::Some(k) => println(k), // 3
        Option::None => println(-1),
    }

    // 3. 覆盖已存在键
    m.insert(2, 222);
    match m.get(2) {
        Option::Some(v) => println(v), // 222
        Option::None => println(-1),
    }
    println(m.len()); // 3

    // 4. remove
    let removed = m.remove(1);
    println(removed); // true
    println(m.contains_key(1)); // false
    println(m.len()); // 2

    // 5. keys / values（有序对齐）
    let ks = m.keys();
    println(ks[0]); // 2
    let vs = m.values();
    println(vs[0]); // 222
}
