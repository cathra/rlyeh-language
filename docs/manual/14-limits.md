# 14. 已知限制

以下为 MVP 已明确**未实现**或**有约束**的特性（编写时务必规避；规范文档 `grammar.md`/`semantics.md` 等的「规划」标注仅表示长期路线图）。

## 14.1 未实现（typecheck 显式报 Unsupported）

- 宏调用（已 ✅ 实现的部分见 §13.3 指南）；无卫生宏（hygiene）
- 高级 FFI 绑定（自动类型映射 / 安全封装）
- `move` 关键字语义（MVP 忽略，所有权宽松）
- 按引用捕获闭包
- 嵌套捕获闭包（外层闭包体内直接写 IIFE 捕获内层可运行）

## 14.2 约束（MVP 限制）

- **模块**：不支持 `pub use` 重导出、相对 `super::`、嵌套模块声明
- **dyn Trait**：非泛型 trait/impl、含 `Self` 签名方法不可经 dyn 调用；vtable 的 drop/size/align 槽置 0
- **闭包**：仅按值捕获；捕获闭包值不跨函数边界（作 fn 实参 / 返回值报 Unsupported）；参数类型有注解用注解、无注解由首次调用点实参推断（半注解亦可用）
- **泛型 `Self`**：仅返回位置；参数位置（关联返回）禁止
- **`as` 转换**：i128/u128、指针/引用/聚合转换保持擦除
- **Actor**：跨编译单元 ask 需运行时符号表（native dlsym / WASI 静态注入）
- **异步**：await 位于控制流块 / 表达式中间、按引用捕获规划中
- **JSON/HashMap**：`map![...]` 绑定后 K/V 为 `Infer`，须显式类型注解；嵌套 `HashMap` 值 parse 报 Unsupported
- **生命周期**：`'a` 语法接受宽松检查，严格生命周期验证规划中
- **切片**：数组范围切片按值拷贝返回 `Vec`；切片引用 `&[T]` 为零拷贝视图，二者语义不同

## 14.3 已实现特性速查（避免误以为缺失）

引用 `&T`/`&mut T`、严格借用检查、`str` 一等类型、字面量实参自动升级、`?`、`as`、`dyn Trait`、闭包 H2/H3/H5、`Box`/`Rc`/`Arc`/`Gc`、Actor + 交叉编译 WASM、`async`/`.await`、`Vec::iter`、`String::chars`/`lines`、迭代器适配器、`#[derive(Serialize, Deserialize)]`、`toml::*`、`future::join_all`/`timeout` 等均已实现。

---

[← 上一章：工具链命令速查](./13-toolchain.md) | [返回手册目录](./index.md)
