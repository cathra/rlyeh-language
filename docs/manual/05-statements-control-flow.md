# 5. 语句与控制流

## 5.1 变量绑定

```rlyeh
let x = 6;                 // 不可变，推断 i64
let mut v = Vec::new();    // 可变绑定（对象内部修改）
let y: f64 = 3.14;         // 显式注解
```

**块级遮蔽**（U1 ✅）：块内 `let x` 覆盖外层同名变量，块内引用指向新绑定，块结束后原变量恢复；函数/闭包体为隔离边界，循环体 / `match` 臂可穿透所属函数层。

## 5.2 控制流

```rlyeh
if cond { } else { }          // if 表达式（尾值）
while cond { }
loop { break; continue; }
for i in 0..<10 { }           // 数值区间（步长 1）
for j in 1...3 { }            // 双闭区间
for x in vec { }              // 容器迭代
match value { ... }           // 模式匹配（枚举解构）
```

`match` 模式：枚举变体 `Enum::Variant(...)`、结构字段 `{ w, h }`、嵌套 `Option<Vec<T>>` 均支持。

## 5.3 赋值与序列

- 赋值 `=` 为表达式（可链式 / 作为值）；`a = b = 5;` 合法
- 块 `{ ... }` 末表达式即块值；`let x = { 1; 2; };` 绑定 2

---

[← 上一章：表达式与运算符](./04-expressions-operators.md) | [返回手册目录](./index.md) | [下一章：函数与闭包 →](./06-functions-closures.md)
