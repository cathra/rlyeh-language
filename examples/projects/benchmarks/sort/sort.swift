// benchmark: LCG 生成 5000 个 i64 排序 —— 与 sort.zeta 同逻辑
let n = 5000
var v = [Int64]()
v.reserveCapacity(n)
var x: Int64 = 12345
for _ in 0..<n {
    x = x &* 6364136223846793005 &+ 1442695040888963407
    v.append(x)
}
v.sort()
print(v[0])
print(v[n - 1])
