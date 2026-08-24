// benchmark: 20 万 insert + 20 万 get（LCG 键）—— 与 hashmap.zeta 同逻辑
var m = Dictionary<Int64, Int64>()
m.reserveCapacity(400000)

var x: Int64 = 12345
for i in 0..<200000 {
    x = x &* 6364136223846793005 &+ 1442695040888963407
    m[x] = Int64(i)
}

var sum: Int64 = 0
var y: Int64 = 12345
for _ in 0..<200000 {
    y = y &* 6364136223846793005 &+ 1442695040888963407
    if let v = m[y] { sum += v } else { sum += 1 }
}
print(sum)
