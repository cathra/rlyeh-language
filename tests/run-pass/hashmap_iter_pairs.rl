// V1-b：HashMap::iter_pairs() 键值对引用迭代器（KVRef 零拷贝，等价 (&K,&V)）
fn main() {
    let mut m: HashMap<i64, i64> = HashMap::new();
    m.insert(1, 10);
    m.insert(2, 20);
    m.insert(3, 30);
    // 1. 遍历求和 key+val，并计数（零拷贝读取原缓冲）
    let mut sum = 0;
    let mut cnt = 0;
    for e in m.iter_pairs() {
        sum = sum + *e.key + *e.val;
        cnt = cnt + 1;
    }
    println(sum); // (1+10)+(2+20)+(3+30) = 66
    println(cnt); // 3
    // 2. 删除一个键（置墓碑 states==2），确认遍历跳过墓碑、只遍历存活槽
    m.remove(2);
    let mut sum2 = 0;
    let mut cnt2 = 0;
    for e in m.iter_pairs() {
        sum2 = sum2 + *e.key + *e.val;
        cnt2 = cnt2 + 1;
    }
    println(sum2); // (1+10)+(3+30) = 44
    println(cnt2); // 2
    // 3. 既有值拷贝路径 keys() 仍可用（不退化）
    let mut ksum = 0;
    for k in m.keys() {
        ksum = ksum + k;
    }
    println(ksum); // 1 + 3 = 4
}
