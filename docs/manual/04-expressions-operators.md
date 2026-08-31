# 4. 表达式与运算符

一切皆为表达式，末表达式即返回值（隐式 return）。

## 4.1 运算符一览

| 类别 | 运算符 |
|------|--------|
| 算术 | `+ - * / %` |
| 比较 | `== != < <= > >=` |
| 逻辑 | `&& || not` |
| 位运算 | `& | ^ << >> ~` |
| 类型转换 | `as`（如 `i64 as f64`，U6 ✅） |
| 错误传播 | `?`（K1 ✅） |
| 解引用 / 引用 | `*` / `&` / `&mut` |
| 成员 / 调用 / 索引 | `.` / `()` / `[]` |

## 4.2 比较链（数学式）

```rlyeh
if 0 < x < 10 {}        // 0 < x && x < 10
if 0 > x > 10 {}        // x < 0 || x > 10
```

## 4.3 集合 / 区间判断

```rlyeh
if x in (1, 3, 5) {}    // x == 1 || x == 3 || x == 5
if x in 0..<10 {}       // 0 <= x < 10
if hour in (9am...6pm) {}  // 时间字面量区间
```

## 4.4 `?` 错误传播（K1）

`expr?` 在 `Option<T>` / `Result<T, E>` 上下文解包成功值，失败早返回：

- `Option<T>` → `match { Some(v) => v, None => return Option::None }`
- `Result<T, E>` → `match { Ok(v) => v, Err(e) => return Result::Err(e) }`

支持表达式中间嵌套 `?`；`?` 用于非 Option/Result 类型报 Unsupported。

## 4.5 `as` 数值转换（U6）

| 转换 | 语义 | 示例 |
|------|------|------|
| f64 ↔ i64 | `fptosi`/`sitofp`（向零截断） | `3.7 as i64` = 3 |
| 整数截断 / 扩展 | `trunc`/`sext`/`zext` | `300 as i8` = 44 |
| 整 ↔ bool | `icmp ne 0` / `zext` | `5 as bool` = true |
| 整 ↔ char | `trunc`/`zext`（char 为 32 位码点） | `'a' as i64` = 97 |
| 同类型 | 零指令 | `42 as i64` = 42 |

范围：≤64 位整族 + 浮点 + bool + char；i128/u128 与指针/引用/聚合转换保持擦除。

## 4.6 `dyn Trait`（H4）

`&T` 强制转换 → vtable + 2 槽胖指针，方法调用经 vtable 间接分派。MVP 限制：非泛型 trait/impl、含 `Self` 签名方法不可经 dyn 调用；vtable 的 drop/size/align 槽置 0。

---

[← 上一章：类型系统](./03-types.md) | [返回手册目录](./index.md) | [下一章：语句与控制流 →](./05-statements-control-flow.md)
