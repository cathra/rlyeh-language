// benchmark: 256x256 f64 矩阵乘法 —— 与 matmul.rl 同逻辑
let n = 256
let total = n * n
var a = [Double](repeating: 1.0001, count: total)
var b = [Double](repeating: 1.0001, count: total)
var c = [Double](repeating: 0.0, count: total)
for x in 0..<n {
    for y in 0..<n {
        var s = 0.0
        for k in 0..<n {
            s += a[x * n + k] * b[k * n + y]
        }
        c[x * n + y] = s
    }
}
var sum = 0.0
for i in 0..<total {
    sum += c[i]
}
print(sum)
