// 对应 docs/guide/04-math-conditions.md —— 数学式条件判断
// 运行：rlyeh run 04-math-conditions.rl
fn classify(hour: i64, temp: f64, tag: i64) {
    // 工作时间且温度舒适（比较链 + 区间）
    if hour in {9am...6pm} && 18.0 < temp < 26.0 {
        println("舒适的工作时段");
    };
    // 离散集合成员判断
    if tag in {404, 500, 503} {
        println("服务端错误");
    };
    // 反向链（区间外）+ not in
    if hour not in {9am...6pm} && tag not in {6, 7} {
        println("空闲");
    };
}

fn main() {
    classify(600, 22.0, 200);
    let x = 5;
    if 0 < x < 10 { println("在区间内"); };      // 正向链 → 区间内
    if 0 > x > 10 { println("在区间外"); };       // 反向链 → 区间外（此处不触发）
}
