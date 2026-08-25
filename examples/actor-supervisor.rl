// actor-supervisor.rl — Supervisor 恢复示例
//
// 演示：
// - `Machine::new_supervised(0)` 语言级受监督构造（strategy 0 = OneForOne）
// - 方法返回 -1 触发崩溃协议（runtime 视为 Panic）
// - supervisor 捕获后调用 factory（__state_new）重建初始状态
// - 重启后 actor 继续可用，状态回到初始值

actor Machine {
    uptime: i64 = 0,

    // tick：uptime += n，返回累计值
    pub fn tick(n: i64) -> i64 {
        self.uptime += n;
        self.uptime
    }

    // explode：返回 -1 → runtime 判定崩溃 → supervisor 重启
    // （注意：任何返回 -1 的 actor 方法都会触发崩溃协议）
    pub fn explode() -> i64 {
        -1
    }
}

fn main() {
    // 0 = OneForOne：崩溃只重启该 actor
    let m = Machine::new_supervised(0);
    println(m.tick(5).await);      // 5
    // 崩溃：ask 立即失败返回 0，supervisor 后台重启
    println(m.explode().await);    // 0
    // 重启后状态重建为初始 uptime = 0，再 tick 3 → 3
    println(m.tick(3).await);      // 3
}
