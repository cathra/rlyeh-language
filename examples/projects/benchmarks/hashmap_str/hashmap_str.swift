// 基准: hashmap_str —— 1 万条字符串键哈希表插入与查询
// 与 hashmap_str.zeta 逻辑严格一致。输出 = 49995000
let n = 10000
var m = [String: Int64]()
m.reserveCapacity(n * 2)
for i in 0..<n {
    m["key_\(i)"] = Int64(i)
}
var sum: Int64 = 0
for i in 0..<n {
    if let v = m["key_\(i)"] {
        sum += v
    } else {
        sum -= 1
    }
}
print(sum)
