# 01 · Option / Result 与错误处理

> 规范：docs/std-lib.md §2.1（Option）/ §2.2（Result）/ §12（错误处理）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `Option<T>` 枚举：`Some(v)` / `None` 构造与 `match` 解构
- `Result<T, E>` 枚举：`Ok(v)` / `Err(e)` 构造与 `match` 解构
- `?` 错误传播运算符：Option / Result 上下文自动解包，失败提前返回
- `Error` protocol + `IoError` / `IoErrorKind`（dyn 分派）

## 示例清单

| 文件 | 说明 |
|------|------|
| `question.rl` | K1 `?` 运算符：链式解包、表达式中间嵌套 `?`、失败提前返回 |
| `error_trait.rl` | M2 `Error` protocol：`impl IoError: Error` + dyn vtable 分派 + 错误转换 |
| `io_result.rl` | IO 操作返回 `Result<i64, IoError>` 的错误处理模式 |

## 运行

```bash
rlyeh run examples/std-demos/01-option-result/question.rl
rlyeh run examples/std-demos/01-option-result/error_trait.rl
rlyeh run examples/std-demos/01-option-result/io_result.rl
```
