// V5 新集合：VecDeque<T> 环形双端队列（2026-08-26）
// 覆盖：new 构造、push_back/push_front、pop_front/pop_back、front/back、len/is_empty
fn main() {
    // 1. push_back / pop_front（FIFO 顺序）
    let mut d: VecDeque<i64> = VecDeque::new();
    d.push_back(1);
    d.push_back(2);
    d.push_back(3);
    match d.pop_front() {
        Option::Some(v) => println(v), // 1
        Option::None => println(-1),
    }
    match d.pop_front() {
        Option::Some(v) => println(v), // 2
        Option::None => println(-1),
    }
    println(d.len()); // 1

    // 2. push_front（头插，LIFO 顺序反转）
    let mut d2: VecDeque<i64> = VecDeque::new();
    d2.push_front(10);
    d2.push_front(20);
    d2.push_front(30);
    match d2.pop_front() {
        Option::Some(v) => println(v), // 30
        Option::None => println(-1),
    }
    match d2.pop_front() {
        Option::Some(v) => println(v), // 20
        Option::None => println(-1),
    }

    // 3. front / back 窥视（不移除）
    let mut d3: VecDeque<i64> = VecDeque::new();
    d3.push_back(5);
    d3.push_back(6);
    d3.push_back(7);
    match d3.front() {
        Option::Some(v) => println(v), // 5
        Option::None => println(-1),
    }
    match d3.back() {
        Option::Some(v) => println(v), // 7
        Option::None => println(-1),
    }

    // 4. pop_back（LIFO 尾出）
    match d3.pop_back() {
        Option::Some(v) => println(v), // 7
        Option::None => println(-1),
    }
    match d3.pop_back() {
        Option::Some(v) => println(v), // 6
        Option::None => println(-1),
    }

    // 5. 空队列 / is_empty
    let mut d4: VecDeque<i64> = VecDeque::new();
    println(d4.is_empty()); // true
    match d4.pop_front() {
        Option::Some(v) => println(v),
        Option::None => println(-1), // -1
    }
    d4.push_back(100);
    println(d4.len()); // 1
    println(d4.is_empty()); // false
}
