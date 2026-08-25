// duration_full.rl：X1 时间 API 完整化
// - Duration 构造器 microseconds/nanoseconds 与读方法 as_secs/as_millis/as_nanos
// - Instant::duration_since（同一时刻差为 0）
// - SystemTime（now 距 unix_epoch 秒数在 2026 年合理范围 + 先后关系）
fn main() {
    // 构造器：微秒（1500us = 1.5ms）
    let d1 = Duration::microseconds(1500);
    println(d1.as_millis()); // 1
    println(d1.as_secs()); // 0
    // 构造器：纳秒（微秒存储，向下取整：2500ns = 2.5us -> 2us）
    let d2 = Duration::nanoseconds(2500);
    println(d2.micros()); // 2
    // 秒/毫秒/纳秒读方法
    let d3 = Duration::seconds(2);
    println(d3.as_millis()); // 2000
    println(d3.as_nanos()); // 2000000000
    // duration_since：同一时刻差为 0
    let t0 = Instant::now();
    println(t0.duration_since(t0).micros()); // 0
    // SystemTime：now 距 UNIX 纪元秒数（2026 年 > 17 亿秒，稳定断言）
    let st = SystemTime::now();
    let ep = st.duration_since(SystemTime::unix_epoch());
    if ep.as_secs() >= 1700000000 {
        println(1)
    } else {
        println(0)
    }
    // SystemTime 先后关系：s2 不早于 s1
    let s1 = SystemTime::now();
    let s2 = SystemTime::now();
    let rel = s2.duration_since(s1).micros();
    if rel >= 0 {
        println(1)
    } else {
        println(0)
    }
}
