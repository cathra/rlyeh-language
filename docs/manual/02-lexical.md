# 2. 词法与字面量

## 2.1 标识符

```
IDENT = [A-Za-z_][A-Za-z0-9_]*
```

## 2.2 字面量

| 类别 | 语法 | 示例 |
|------|------|------|
| 整数 | `[0-9]+`（默认 i64） | `42`, `0xFF`（十六进制）, `0o17`（八进制）, `0b1010`（二进制） |
| 浮点 | `[0-9]+\.[0-9]+`（f64） | `3.14`, `1e10`, `2.5E-3` |
| 布尔 | `true` / `false` | — |
| 字符 | 单引号（32 位 Unicode 码点） | `'a'`, `'€'`, `'\n'`（转义） |
| 字符串 | 双引号（UTF-8 字节序列） | `"hello"` |
| 原始字符串 | `r#"..."#`（任意 `#` 定界，无转义，与 `r#ident` 区分） | `r#"a"b"c"#` |
| 时间 | 分钟单位（`9am`=540 / `6pm`=1080） | `9am`, `6pm` |
| 区间 | 半开 `..<` / 双闭 `...` | `0..<10`, `0...10` |
| 数组 | `[...]` | `[1, 2, 3]` |
| 单元 | `()` | — |

## 2.3 关键字与保留字

```
fn let mut pub const struct enum trait impl module import if else while loop for
in break continue match return async await send actor region transfer self
true false type extern as ref
```

`Str` 类型保留（未实现，勿用）；`move` 关键字 MVP 忽略（所有权宽松）。

## 2.4 注释

```rlyeh
// 行注释
/// 文档注释（rlyeh doc 提取生成 Markdown）
```

## 2.5 运算符字符

`+ - * / % = == != < <= > >= && || ! not & | ^ << >> ~ ? as @ => . .. ... ..< : ; , ( ) { } [ ]`

---

[← 上一章：语言概述](./01-overview.md) | [返回手册目录](./index.md) | [下一章：类型系统 →](./03-types.md)
