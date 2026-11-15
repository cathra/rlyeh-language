// VecDeque::iter() 只读引用迭代器（V5+，零拷贝，从逻辑头 front 开始）。
fn main() {
    let mut d: VecDeque<i64> = VecDeque::new();
    d.push_back(1);
    d.push_back(2);
    d.push_back(3);
    d.pop_front();          // front=1，逻辑元素 [2,3]
    d.push_back(4);         // 逻辑元素 [2,3,4]
    let mut sum = 0;
    let mut cnt = 0;
    for x in d.iter() {
        sum = sum + *x;
        cnt = cnt + 1;
    }
    println(sum);           // 9
    println(cnt);           // 3
}
