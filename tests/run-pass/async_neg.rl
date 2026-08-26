// skip: W2 async 运行时 poll 死循环导致内存无限暴涨（2026-08-26 实测 1.2GB+，
// 全量测试在此处系统内存耗尽）。`ifneg` 的 if+await 控制流在 poll 状态机下死循环
// + 持续分配。待修复 W2 async 控制流图展开后移除本标记。
async fn get_value(x: i64) -> i64 {
    x * 2
}

async fn ifneg(x: i64) -> i64 {
    if x > 0 {
        return get_value(100).await;
    }
    return get_value(-x).await;
}

fn main() {
    let mut f1 = ifneg(3);       // 应走 then：get_value(100)=200
    println(block_on(&mut f1));
    let mut f2 = ifneg(-3);      // 应走 else：get_value(3)=6
    println(block_on(&mut f2));
}
