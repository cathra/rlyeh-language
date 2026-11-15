# 14. 已知限制

> 本章列出 0.2.0 已明确**未实现**或**有约束**的特性，编写时务必规避。规范文档 `grammar.md`/`semantics.md` 等的「规划」标注仅表示长期路线图，不代表当前可用。
>
> **快速判断原则**：指南与本文档（manual/std）**只描述已实现项**；若某功能在本文出现，即可用；若只出现在 `grammar.md` 的「规划」标注里，则暂不可用。

---

## 14.1 未实现 / 移交 0.3.0

| 特性 | 说明 |
|------|------|
| 智能指针 Drop | `Box`/`Rc`/`Arc` 未接 Drop；用户 struct、字段级 drop glue、MutexGuard 自动解锁已支持 |
| `!` never 类型 | `panic!` 当前返回 `()`；`if` 分支类型合并不把 panic 视为发散 |
| enum derive / derive 扩展 | 仅 struct 的 `Clone`/`PartialEq`/`Debug`；enum derive、泛型 bound 强制校验、自定义 derive 扩展点未做 |
| 泛型 impl 关联静态方法 | 泛型关联静态方法暂不支持；Arena 可在调用方构造 |
| 跨线程移动限制 | `Sender`/`Receiver` 类型层/共享已完成；跨线程移动受 `Thread::start` 传参限制 |
| 三阶段 bootstrap | 差分 harness + C0/C1/C2 快照基线完成；K-M1..M3 工具链自举属 0.3.0 |
| 可见性默认私有 | `--visibility=off\|warn\|error` 灰度可用；std 跨模块 `pub` 迁移与默认 `error` 暂缓 |
| Windows sendfile | Y3 保留 Unsupported |
| 宏卫生 / 高阶 FFI | 宏展开不保证 hygiene；高阶 FFI 自动类型映射仍规划中 |
| 外部 crate 等价 | pubgrub/clap/serde/toml/tar/flate2/libc 长期保留 Rust 经 FFI |

## 14.2 约束（当前版本）

| 领域 | 约束 |
|------|------|
| **dyn Protocol** | `Box<dyn Any + Send>` / 多 protocol 约束未实现；完全擦除的 `dyn` 仍按 object-unsafe 拒绝 |
| **闭包** | 捕获闭包可跨函数边界/`move` 跨线程（捕获须 `static`）；按引用捕获与嵌套捕获闭包仍规划中 |
| **`as` 转换** | 指针/引用/聚合转换保持擦除（不支持）；i128/u128 转换有限 |
| **Actor** | 跨编译单元 ask 需运行时符号表（native `dlsym` / WASI 静态注入） |
| **JSON/HashMap** | `map![...]` 绑定后 K/V 为 `Infer` 须显式注解；嵌套 `HashMap` 值 parse 报 Unsupported |
| **生命周期** | `'a` 语法接受；严格生命周期验证仍在演进（当前为块级借用模型） |
| **切片** | 数组范围切片按值拷贝返回 `Vec`；切片引用 `&[T]` 为零拷贝视图，二者语义不同 |
| **无堆模式** | `--no-heap` 禁止堆类型/宏/region；聚合按值跨函数逃逸会被 IR 守卫拒绝，需传 `&`/`&mut` |

## 14.3 已实现特性速查（避免误以为缺失）

以下均已实现，可放心使用：

- 引用 `&T`/`&mut T`、严格借用检查、`str`/`&str`、字面量实参自动升级
- 泛型 protocol/impl、嵌套模块、`pub use`、组导入/glob、`#[derive(Clone/PartialEq/Debug)]`
- `unsafe` 块 / 裸指针 / `#[repr(C)]`、`extern "C"` FFI、`dyn Protocol` 含 `Self` 与 `Any` 下转换
- 元组值/解构、`if let`/`while let`、`match` 守卫 + 范围/或模式、`Drop`/RAII
- `const`/`static`、`mem::swap/replace/take`、`?` 经 `From` 转换、`try { .. }` 错误聚合块
- 闭包 H2/H3/H5、跨函数边界与 `move`、`Box`/`Rc`/`Arc`/`Gc`
- Actor + 交叉编译 WASM、`async`/`.await`、原子类型/线程/Channel
- 标准库：String / Vec / HashMap / BTreeMap / VecDeque / Option / Result / 迭代器
- 编译器：`-O0/-O1/-O2/-O3/-Os`、`--no-heap`、`--visibility` 灰度、`--emit` 中间表示

---

## 更多示例

`--no-heap` 下使用堆容器的典型诊断：

```rlyeh
fn main() {
    let s = String::from("secret");   // ❌ NH001: no-heap mode
}
```

更多约束见 [指南 §13 参考与已知限制](../guide/13-references-limits.md) 与 [`examples/by-chapter/13-references-limits.rl`](../../examples/by-chapter/13-references-limits.rl)。

---

[← 上一章：工具链命令速查](./13-toolchain.md) | [返回手册目录](./index.md)
