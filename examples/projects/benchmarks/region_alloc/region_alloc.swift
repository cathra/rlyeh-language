// 基准: region_alloc —— 100 万次小对象分配（class 堆分配 + ARC 释放）
// 与 region_alloc.rl 逻辑严格一致。输出 = 499500000
// 注: Rlyeh 侧用 region 批量分配（区域退出一次释放），本侧为 class 实例化 + ARC。
final class Big {
    let a: Int64, b: Int64, c: Int64, d: Int64
    init(_ a: Int64, _ b: Int64, _ c: Int64, _ d: Int64) {
        self.a = a; self.b = b; self.c = c; self.d = d
    }
}

var sum: Int64 = 0
for i in 0..<1_000_000 {
    let o = Big(Int64(i % 1000), Int64(i), Int64(i * 2), Int64(i * 3))
    sum += o.a
}
print(sum)
