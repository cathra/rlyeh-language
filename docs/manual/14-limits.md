# 14. 已知限制

> 本章列出 MVP 已明确**未实现**或**有约束**的特性，编写时务必规避。规范文档 `grammar.md`/`semantics.md` 等的「规划」标注仅表示长期路线图，不代表当前可用。
>
> **快速判断原则**：指南与本文档（manual/std）**只描述已实现项**；若某功能在本文出现，即可用；若只出现在 `grammar.md` 的「规划」标注里，则暂不可用。

---

## 14.1 未实现（typecheck 显式报 Unsupported）

| 特性 | 说明 |
|------|------|
| 无卫生宏（hygiene） | 宏调用（`macro_rules!` / `println!` / `vec!` 等）已 ✅，但宏展开不保证标识符卫生 |
| 高级 FFI 绑定 | 自动类型映射 / 安全封装规划中；当前为低阶 `extern "C"` 直接对齐 |
| `move` 关键字语义 | MVP 忽略（所有权宽松）；写了不报错但无激活效果 |
| 按引用捕获闭包 | 闭包仅按值捕获（H3/H5）；按引用捕获规划中 |
| 嵌套捕获闭包 | 外层闭包体内直接写 IIFE 捕获内层可运行，但嵌套捕获闭包本身不支持 |

---

## 14.2 约束（MVP 限制）

| 领域 | 约束 |
|------|------|
| **模块** | 不支持 `pub use` 重导出、相对 `super::`、嵌套模块声明 |
| **dyn Trait** | 非泛型 trait/impl、含 `Self` 签名方法不可经 dyn 调用；vtable 的 drop/size/align 槽置 0 |
| **闭包** | 仅按值捕获；捕获闭包值不跨函数边界（作 fn 实参 / 返回值报 Unsupported）；参数类型有注解用注解、无注解由首次调用点实参推断（半注解亦可用） |
| **泛型 `Self`** | 仅返回位置；参数位置（关联返回）禁止 |
| **`as` 转换** | i128/u128、指针/引用/聚合转换保持擦除（不支持） |
| **Actor** | 跨编译单元 ask 需运行时符号表（native `dlsym` / WASI 静态注入） |
| **异步** | await 位于控制流块 / 表达式中间、按引用捕获规划中 |
| **JSON/HashMap** | `map![...]` 绑定后 K/V 为 `Infer`，须显式类型注解；嵌套 `HashMap` 值 parse 报 Unsupported |
| **生命周期** | `'a` 语法接受、宽松检查，严格生命周期验证规划中 |
| **切片** | 数组范围切片按值拷贝返回 `Vec`；切片引用 `&[T]` 为零拷贝视图，二者语义不同 |

> **C 对照理解**：很多限制对应"C 里你也是手写/自己负责"的部分——例如 `move` 语义宽松 ≈ C 里所有指针都是裸的、靠自觉；生命周期宽松检查 ≈ C 完全不检查。Rlyeh 会逐步收紧这些检查（路线图中），但 MVP 优先"能跑"。

---

## 14.3 已实现特性速查（避免误以为缺失）

以下**均已实现**，可放心使用：

- 引用 `&T`/`&mut T`、严格借用检查、`str` 一等类型、字面量实参自动升级
- `?` 错误传播、`as` 数值转换、`dyn Trait`
- 闭包 H2（无捕获）/ H3（捕获 IIFE）/ H5（闭包值对象）
- `Box`/`Rc`/`Arc`/`Gc`
- Actor + 交叉编译 WASM、`async`/`.await`
- `Vec::iter`、`String::chars`/`lines`、迭代器适配器（`map`/`filter`/`fold`/...）
- `#[derive(Serialize, Deserialize)]`、`toml::*`、`future::join_all`/`timeout`

---

## 更多示例

体验"限制"：故意写一个 MVP 不支持的嵌套模块，用 `rlyeh check` 看报什么：

```rlyeh
module a {
    module b { }   // ❌ 嵌套模块声明：MVP 不支持，typecheck 报 Unsupported
}
```

更多约束见 [指南 §13 参考与已知限制](../guide/13-references-limits.md) 与 [`examples/by-chapter/13-references-limits.rl`](../../examples/by-chapter/13-references-limits.rl)。

---

[← 上一章：工具链命令速查](./13-toolchain.md) | [返回手册目录](./index.md)
