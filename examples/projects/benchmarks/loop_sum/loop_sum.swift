// benchmark: 1 亿次 i64 循环累加 —— 与 loop_sum.rl 同逻辑
var s: Int64 = 0
var i: Int64 = 0
while i < 100000000 {
    s += i
    i += 1
}
print(s)
