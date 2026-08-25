// 基准: actor_pingpong —— 5 万次线程间同步往返（NSCondition 双槽握手）
// 与 actor_pingpong.rl 逻辑严格一致。输出 = 1250025000
import Foundation

let N = 50000
let cond = NSCondition()
var slotAReady = false, slotBReady = false
var slotA: Int64 = 0, slotB: Int64 = 0

let worker = Thread {
    var count: Int64 = 0
    for _ in 0..<N {
        cond.lock()
        while !slotAReady { cond.wait() }
        slotAReady = false
        count += 1
        slotB = count
        slotBReady = true
        cond.signal()
        cond.unlock()
    }
}
worker.start()

var sum: Int64 = 0
for i in 0..<N {
    cond.lock()
    slotA = Int64(i)
    slotAReady = true
    cond.signal()
    while !slotBReady { cond.wait() }
    sum += slotB
    slotBReady = false
    cond.unlock()
}
print(sum)
