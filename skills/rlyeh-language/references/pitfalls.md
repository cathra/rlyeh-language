# Rlyeh 0.2.0 编写陷阱清单（每次写代码前必读）

> 当前版本 v0.2.0。以下条目来自 `docs/guide/13-references-limits.md` 与编译器实际行为；违反限制会得到 `Unsupported`、`NH001` 或链接错误。

## A. 程序结构

1. **必须有 `fn main()`**；否则链接报 `Undefined symbols: _main`。
2. 语句用 `;` 结束；`if`/`while`/`loop` 条件无需括号。
3. `println(expr)` 是内建函数（单参数、自动按类型）；`println!("{}", x)` / `format!` 是宏，占位符写法与 Rust 类似。
4. 宏可用：`format!`、`println!`、`vec!`、`map!`、`arr!`、`assert!`、`panic!` 等；宏展开不保证 hygiene。

## B. 当前未实现 / 限制

5. `Box`/`Rc`/`Arc` 未接 Drop；不要依赖它们自动释放内部资源。
6. `!` never 类型未实现；`panic!` 返回 `()`，不能用于分支类型收敛。
7. enum derive、泛型 bound 强制校验、自定义 derive 扩展点未实现；仅 struct 的 `Clone`/`PartialEq`/`Debug`。
8. 泛型 impl 关联静态方法暂不支持；Arena 在调用方构造。
9. `Sender`/`Receiver` 跨线程移动受 `Thread::start` 传参限制。
10. 默认私有 + `pub` 校验仍是灰度（`--visibility`，默认 `off`）；std 跨模块访问暂未全量迁移。
11. `--no-heap` 下禁止 `String`/`Vec`/`HashMap`/`Box`/`format!`/`region`；聚合按值跨函数逃逸会被 IR 守卫拒绝，改传 `&`/`&mut`。

## C. 语义陷阱

12. **位运算优先级低于比较**：`*` > `+` > `<<` > `&` > `^` > `|`，比较高于位运算。裸 `x & 3 == 2` 会按 `x & (3 == 2)` 解析——必须写 `(x & 3) == 2`。
13. String 索引按**字节**（UTF-8 边界由调用方保证）；中文等多字节字符不能按字符下标直接切。
14. 动态切片 `v[lo..<hi]` 返回**新缓冲**（值拷贝）；切片引用 `&[T]` 是零拷贝视图，两者语义不同。
15. 数组/Vec 赋值是槽拷贝、底层缓冲可能共享；`String` 的 `+` 已深拷贝隔离。
16. 切片/索引越界可能 clamp 或 abort；不要依赖越界写。

## D. Actor

17. 方法返回 **`-1` = 崩溃信号**：ask 返回 0、supervisor 重启、无监督停止；正常业务不要返回 -1。
18. 同步 `.method(args).await`；fire-and-forget `send actor.method(args)`。
19. `new_supervised(mode)`：0=OneForOne、1=AllForOne、2=RestartForOne。

## E. FFI / 链接

20. `extern fn` 符号名必须与 libc/系统库完全一致；模块前缀改名会导致链接失败。
21. 链接 `_main` undefined → 检查 `fn main()`。
22. extern 符号 undefined → 检查签名类型与系统库原型。

## F. 编译错误定位

| 阶段 | 特征 | 常见原因 |
|------|------|----------|
| Lexer | 非法字符 / 未闭合注释 | 不支持的符号、字符串未闭合 |
| Parser | 语法错误（行/列） | 括号/分号/模式形状错误 |
| Typecheck | 类型不匹配 / `Unsupported` / `NH001` | 类型写错、当前限制、无堆模式禁用项 |
| Codegen/LLVM | 符号 undefined / 链接失败 | 缺 main、extern 不一致、目标 triple |

## G. 风格与验证

- 快速验证：`rlyeh run file.rl`；项目：`rlyeh new` + `rlyeh build`。
- 默认 `-O3`；调试用 `-O0`，体积优先 `-Os`；切换级别会使用不同缓存键。
- 需要无堆库时用 `--no-heap`，先确认未使用堆类型/宏/region。
- 逻辑运算 `not x`；位运算加括号；复合类型 match 写全变体。
- 标准库 API 以 `references/std-lib.md` 的 ✅ 项为准。
