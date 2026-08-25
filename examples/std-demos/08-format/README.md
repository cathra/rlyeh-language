# 08 · 格式化与打印

> 规范：docs/std-lib.md §8（格式化与打印）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `println!` / `print!` / `format!` / `dbg!` / `eprintln!` / `eprint!`
- `{}` 值占位符：优先查 `Display` impl（`fmt` 方法），内建类型走内建转换
- `{:?}` 调试占位符：查 `Debug` impl（`fmt_debug`）
- `Display` / `Debug` trait + `Formatter` 自定义实现
- stderr 输出（`eprintln!` 经 POSIX `dprintf(2)` 直写）

## 示例清单

| 文件 | 说明 |
|------|------|
| `display_fmt.rl` | Q3 Display/Debug trait + Formatter + `{}` / `{:?}` 占位 |
| `eprintln.rl` | stderr 输出：eprintln! / eprint! |

## 运行

```bash
rlyeh run examples/std-demos/08-format/display_fmt.rl
rlyeh run examples/std-demos/08-format/eprintln.rl
```
