// V4 `get_mut` 引用语义：Vec/HashMap 返回 `Option<&mut T>`，经 `*r = x` 写回真实槽。
fn main() {
    // 1. Vec::get_mut：命中写回真实槽
    let mut v: Vec<i64> = vec![1, 2, 3];
    match v.get_mut(0) {
        Option::Some(r) => *r = 100,
        Option::None => println(-1),
    }
    println(v[0]); // 100
    println(v[1]); // 2

    // 2. Vec::get_mut：越界返回 None（不 panic）
    match v.get_mut(5) {
        Option::Some(r) => *r = 0,
        Option::None => println(-1), // -1
    }
    match v.get_mut(-1) {
        Option::Some(r) => *r = 0,
        Option::None => println(-1), // -1
    }

    // 3. 连续写回：二次 get_mut 读回已写值
    let mut w: Vec<i64> = vec![10, 20, 30];
    match w.get_mut(2) {
        Option::Some(r) => *r = *r + 5, // 30 -> 35
        Option::None => println(-1),
    }
    println(w[2]); // 35
    match w.get_mut(2) {
        Option::Some(r) => *r = *r * 2, // 35 -> 70
        Option::None => println(-1),
    }
    println(w[2]); // 70

    // 4. HashMap::get_mut：命中写回真实槽，get 读回新值
    let mut m: HashMap<i64, i64> = map![1 => 100, 2 => 200, 3 => 300];
    match m.get_mut(2) {
        Option::Some(r) => *r = 999,
        Option::None => println(-1),
    }
    match m.get(2) {
        Option::Some(x) => println(x), // 999
        Option::None => println(-1),
    }

    // 5. HashMap::get_mut：缺失键返回 None
    match m.get_mut(9) {
        Option::Some(r) => *r = 0,
        Option::None => println(-1), // -1
    }

    // 6. HashMap 写回后长度不变、其他键不受影响
    println(m.len()); // 3
    match m.get(1) {
        Option::Some(x) => println(x), // 100
        Option::None => println(-1),
    }

    // 7. 引用穿透：get_mut 返回值可继续经引用读（剥层）
    let mut v7: Vec<i64> = vec![5, 6, 7];
    match v7.get_mut(1) {
        Option::Some(r) => println(*r), // 6
        Option::None => println(-1),
    }
}
