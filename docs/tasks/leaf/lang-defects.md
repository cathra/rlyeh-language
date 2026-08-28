# 语言级缺陷跟踪专项（Lang Defects）

> **所属阶段**：阶段 Y（专项跟踪文档，非独立阶段）
> **状态**：📋 待办（登记探测到的语言级类型系统/泛型/dyn 缺陷，统一评估后修复）
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

集中跟踪 Rlyeh 语言级缺陷（类型系统、泛型、dyn trait 对象、切片参数化等），按规则：**探测到的新语言级缺陷先记录到本文档**，评估影响后再统一修复。区别于 [`parser-rework.md`](./parser-rework.md)（内建解析器重构专项）。

## 背景

Y 阶段任务风险评估（2026-08-28）在实测中探测到多个语言级缺陷，阻碍 Y4/Y6/Y1 等任务的目标签名落地。

## 已登记缺陷

### 1. 泛型 struct 字面量构造无法实例化（源自 Y4）

- **症状**：`struct MutexGuard<T> { value: T }` + `let g: MutexGuard<i64> = MutexGuard { value: 42 };` → typecheck 报 `argument 0 of struct MutexGuard field value expects T, found i64`。
- **即使显式类型注解 `MutexGuard<i64>` 仍失败**——`T` 未被替换为具体类型。
- **影响**：Y4 的 `Mutex<T>`/`MutexGuard<T>` 泛型锁、`Channel<T>` 泛型化的**字面量构造路径**不可用。std 泛型类型（`Vec<T>`/`Result<T,E>`/`HashMap<K,V>`）经内建/特判构造，用户自定义泛型 struct 缺泛型实例化支持。
- **修复方向**：typecheck 泛型 struct 字面量构造时实例化泛型参数（从类型注解或字段实参推断 `T`）。

### 2. `&dyn Error` trait 对象构造/上转型失败（源自 Y6）

- **症状**：
  - `let d: dyn Error = e;`（e: MyError）→ `expected dyn Error, found MyError`（无自动上转型）
  - `let d: &dyn Error = r;`（r: &MyError）→ `expected &dyn Error, found &MyError`（引用也不能转 dyn）
- **影响**：Y6 的 `source() -> Option<&dyn Error>` 目标签名虽可声明，但无法构造非 `None` 的 `&dyn Error` 值（链式 source 语义不可实现）。
- **修复方向**：支持具体类型 → dyn trait 对象的上转型（`&T as &dyn Trait` / 自动 coercion）。

### 3.（预留）切片参数化（U1，源自 Y1）

- `read(&mut [u8])`/`write(&[u8])` 切片实参依赖 U1 切片成熟——`&mut [u8]`/`&[u8]` 作函数参数 + 切片值传递，待确认语言支持后登记。

### 4. 泛型 trait 实参不支持路径 + 无 where 子句（源自 Y6b）

- **症状**：`impl From<io::error::IoErrorKind> for IoError` → parser 报 `expected '>', found Colon`（泛型类型实参不支持 `::` 路径）；裸名 `impl From<IoErrorKind>` 可行。
- **影响**：Y6b 的 `From`/`Into` std 层只能对**裸名**类型生效；模块内路径类型（`io::error::IoErrorKind`）的泛型 impl 不可写。
- **where 子句缺失**：`impl<T, U> Into<U> for T where U: From<T>`（blanket impl）不可行——无 `where` 子句语法。
- **修复方向**：parser 泛型类型实参支持 `::` 路径 + 增加 `where` 子句（`?` 运算符 From 自动转换前提）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 建立专项文档；登记泛型 struct 字面量构造障碍（Y4）+ `&dyn Error` 构造障碍（Y6） |
| 2026-08-28 | 登记泛型 trait 实参路径不支持 + where 子句缺失（Y6b） |
