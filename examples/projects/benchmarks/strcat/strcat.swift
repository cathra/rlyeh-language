// benchmark: String += 10 万次 —— 与 strcat.rl 同逻辑
var s = ""
for _ in 0..<100000 {
    s += "ab"
}
print(s.count)
