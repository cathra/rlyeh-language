# Rlyeh MVP 编写陷阱清单（每次写代码前必读）

> MVP（v0.1.0）语法边界严格。以下条目来自 `docs/guide/13-references-limits.md` §13 已知限制与编译器实际行为。
> 违反任一「语法不支持」条目 → typecheck 报 `Unsupported` 或链接失败。

## A. 程序结构（最常见错误）

1. **必须有 `fn main()`**。否则链接报 `Undefined symbols: _main`。Rlyeh 无隐式入口。
2. 语句用 `;` 结束；`if`/`while`/`loop` 条件**无需括号**。
3. `println(expr)` 是**内建函数不是宏**：单参数、自动按类型输出、**不支持 `{}` 占位符**。
4. **无宏调用语法**：`println!` / `vec!` / `format!` 全部不可用（`!` 是 `not` 一元运算符）。

## B. 语法不支持（parser 可解析、typecheck 报 Unsupported）

5. **引用**：`&x` 表达式、`&T` 参数、`str` 类型、解引用 `*`、裸指针——全部未实现。
   仅方法接收者 `&self` / `&mut self` 可用。
6. **闭包**：`|x| x + 1` 语法可解析但 Unsupported。
7. `dyn Trait`、函数指针、`?` 错误传播——未实现。
8. `Rc<T>` / `Arc<T>` / `Gc<T>`（L2/L3 内存层）——规划未实现。
9. **迭代器**：`Iterator` protocol / `collect` 未实现；`for` **仅支持数值区间**（`for i in 0..<10`），不支持 `for x in vec`。
10. 字符串插值 / `format!` 格式化——未实现。

## C. 语义陷阱（可编译但行为不同）

11. **位运算优先级低于比较**：优先级 `*` > `+` > `<<` > `&` > `^` > `|`，比较高于位运算。
    裸 `x & 3 == 2` 会解析为 `x & (3 == 2)`（bool）——**必须写 `(x & 3) == 2`**。
12. `String::from(s)`：仅接受字面量或绑定字面量的变量；非字面量 Str 长度表达未实现。
13. String 按**字符**索引、步长 1 字节（ASCII 假设）；中文等多字节字符索引会错位。
14. 动态切片返回**全新缓冲**（值拷贝），修改切片不影响原数组。
15. 数组/Vec 绑定与赋值是 3 槽拷贝、**底层缓冲共享**（别名可见）；拼接 `a + b` 已修复为深拷贝、隔离安全。
16. 数组索引越界在编译期可查（字面量场景）；动态切片越界 clamp 而非报错。

## D. Actor 陷阱

17. 方法返回 **`-1` = 崩溃信号**：ask 立即返回 0、supervisor 重启、无监督则停止——**正常业务不要返回 -1**。
18. 同步调用用 `.method(args).await`；异步 fire-and-forget 用 `send actor.method(args)`。
19. 受监督 spawn 用 `Counter::new_supervised(0)`（0=OneForOne 1=AllForOne 2=RestartForOne），崩溃后重建初始状态重启。

## E. FFI / 链接

20. `extern fn` 符号名必须与 libc/系统库**完全一致**（模块/import 前缀改名 → 链接失败）。
21. 链接报 `_main` undefined → 检查源文件是否有 `fn main()`。
22. 链接报 extern 符号 undefined → 检查签名类型与 libc 是否一致（如 `u32` vs `c_uint`）。

## F. 编译错误快速定位

| 报错阶段 | 特征 | 常见原因 |
|----------|------|----------|
| Lexer | 非法字符 / 未闭合注释 | 用了不支持的符号（如 `@` `#` 宏语法） |
| Parser | 语法错误（行/列） | 括号/分号缺失、表达式形状错误 |
| Typecheck | 类型不匹配 / `Unsupported` | 用了 B 类语法、类型写错 |
| Codegen/LLVM | 符号 undefined / 链接失败 | 缺 main、extern 符号不一致、目标 triple 问题 |

## G. 编写风格建议

- 优先用 `rlyeh run <file.rl>` 快速验证；复杂项目用 `rlyeh new` + `rlyeh build`。
- 逻辑运算：`not x`（不是 `!x`，虽然 `!` 也是 not 但结合易错）；`&&` / `||` 正常。
- 常量用 `const`；需要可变量用 `let mut`。
- 复合类型（struct/enum/impl）用 `match` 解构时写全变体模式。
- 标准库只调 `references/std-lib.md` 中 ✅ 已实现 API，规划 API 编译不过。
