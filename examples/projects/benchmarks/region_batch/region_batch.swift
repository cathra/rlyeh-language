// 对照实现: region_batch.swift —— 手动 bump（与 region 语义对齐：一次性预分配 + 线性分配）
// UnsafeMutablePointer 读写为 Swift 显式内存操作（编译器保守保留），
// escape() 黑盒读强制真实内存带宽，对齐 region_batch.zeta。
// 输出 = 2000497500000
import Foundation

struct Big {
    var a: Int64
    var b: Int64
    var c: Int64
    var d: Int64
}

@inline(never)
func escape(_ p: UnsafeMutablePointer<Big>) {
    var s: UInt8 = 0
    for k in 0..<MemoryLayout<Big>.stride {
        s &+= p.withMemoryRebound(to: UInt8.self, capacity: MemoryLayout<Big>.stride) { $0[k] }
    }
}

let total = 1_000_000 * 4 * MemoryLayout<Big>.stride
var buf = [UInt8](repeating: 0, count: total)
var sum: Int64 = 0
buf.withUnsafeMutableBytes { raw in
    var cur = raw.baseAddress!.assumingMemoryBound(to: Big.self)
    for i in Int64(0)..<1_000_000 {
        let x1 = cur; cur = cur.advanced(by: 1)
        let x2 = cur; cur = cur.advanced(by: 1)
        let x3 = cur; cur = cur.advanced(by: 1)
        let x4 = cur; cur = cur.advanced(by: 1)
        x1.pointee = Big(a: i % 1000, b: i, c: i * 2, d: i * 3)
        x2.pointee = Big(a: i, b: i + 1, c: i + 2, d: i + 3)
        x3.pointee = Big(a: i * 2, b: i, c: i, d: i)
        x4.pointee = Big(a: i, b: i, c: i, d: i)
        sum += x1.pointee.a + x2.pointee.a + x3.pointee.a + x4.pointee.a
        escape(x4)
    }
}
print(sum)
