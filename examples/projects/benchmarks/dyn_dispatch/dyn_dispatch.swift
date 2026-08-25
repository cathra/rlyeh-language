// 基准: dyn_dispatch —— 2000 万次多态分派（protocol 存在类型 witness table）
// 与 dyn_dispatch.rl 逻辑严格一致。输出 = 70000000
protocol Shape {
    func sides() -> Int64
}

struct Tri: Shape {
    let n: Int64
    func sides() -> Int64 { n + 3 }
}

struct Quad: Shape {
    let n: Int64
    func sides() -> Int64 { n + 4 }
}

let d1: any Shape = Tri(n: 0)
let d2: any Shape = Quad(n: 0)
var sum: Int64 = 0
for _ in 0..<10_000_000 {
    sum += d1.sides()
    sum += d2.sides()
}
print(sum)
