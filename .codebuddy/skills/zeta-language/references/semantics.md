# Zeta 语义要点（v0.1.0 MVP）

> 权威规范：`docs/semantics.md`、`docs/memory-model.md`、`docs/actor-model.md`。本节解释**运行时行为**，
> 用于调试与代码审查。

## 1. 值语义与缓冲共享

- String / Vec 是 3 槽值（`ptr`/`len`/`cap`）。`let b = a` 拷贝 3 槽，但**底层缓冲共享**：
  - 数组别名：`let b = arr; arr[1] = 99;` 对 `b` 可见。
  - 拼接已修复：`a + b` desugar 为 `a.clone().push_str(b)`（深拷贝），**结果与左操作数隔离**，`push_str` 不会再污染左操作数。
- 动态切片（`arr[1..<3]` / `v[lo...hi]`）返回**全新缓冲**（元素按值拷贝），是值不是引用/视图。

## 2. 比较链语义

| 写法 | 语义 |
|------|------|
| `0 < x < 10` | `(0<x) && (x<10)` 正向区间链 |
| `0 <= x <= 10` | 双闭区间 |
| `0 > x > 10` | `(x<0) \|\| (x>10)` 反向链（区间外） |
| `x in (1, 3, 5)` | 离散集合，展开为 `==` 链 |
| `ch in ('a'..<'z')` | 范围元素展开为离散成员 |
| `x in 0..<10` | 裸范围 = 区间判断 `[0, 10)` |
| `x in 0<..10` | `(0, 10]` |
| `x not in (6am..<10pm)` | 跨午夜（`9am`=540、`6pm`=1080 分钟整数） |

## 3. 索引与越界

- 数组字面量越界在**编译期**可查。
- 动态切片越界自动 **clamp** 到 `[0, len]`；`start >= end` 返回空。
- String 按**字符**索引，步长 1 字节（ASCII 假设）。

## 4. Region 语义

- `region 'r { ... }`：块结束**批量释放**；`in 'r` 分配进区域。
- `return transfer d out of 'r;`：所有权转移出区域（返回值）。
- `region 'r adaptive`：编译器自动推断大小（可用 PGO 画像回灌初始容量，见 `zeta profile`）。

## 5. Actor 协议

- 消息经「**kind 槽 + 3 个 i64 槽**」传递；同一 actor 的消息按**邮箱 FIFO 互斥**处理。
- 方法返回 **`-1` = 崩溃信号**：
  - ask（`.await`）立即返回 `0`；
  - 受监督 actor（`new_supervised(0|1|2)`）由 supervisor 经 `__state_new` 重建初始状态并重启（0=OneForOne，1=AllForOne，2=RestartForOne）；
  - 无监督 actor 停止。
- `.await` = ask 同步往返；`send actor.method(x)` = fire-and-forget（异步）。
- actor 崩溃协议要求方法返回 `i64`（`-1` 是保留崩溃值，正常业务不要返回 -1）。

## 6. 类型系统行为

- 泛型**单态化**（无擦除、无 trait 对象、无 `dyn`）。
- `match` 枚举解构按**具体实例化**处理（含嵌套泛型 `Option<Vec<T>>`、`Result<Option<String>, i64>`）。
- 裸 `Ok`/`Err`/`Some` 构造 + `unwrap_or` 等可自动定型（Infer 回填）。
- `String::from(s)`：仅接受字面量或绑定字面量的变量（非字面量 Str 长度表达未实现）。

## 7. FFI 行为

- `extern fn` 生成 LLVM `declare`（链接器解析），普通 fn 生成 `define`。
- extern 符号名必须与 libc/系统库**完全一致**（模块/use 前缀改名会导致链接失败——标准库的全部 extern 因此集中在根模块）。
- 未知名类型在 extern 签名中回退为 `Ptr`。

## 8. 常量折叠与编译期检查

- 位运算、算术支持常量折叠（编译期可算出字面量结果）。
- `zeta check` 静态分析：未使用变量 / 恒常条件 / 冗余比较 / 不可达代码。
