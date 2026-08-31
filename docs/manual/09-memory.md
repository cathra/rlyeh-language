# 9. 内存模型

Rlyeh 采用分层内存管理，从零开销到可选 GC 逐级递进：

| 层级 | 机制 | 关键字 | 运行时开销 |
|------|------|--------|------------|
| L0 | 静态所有权 + 借用 + 生命周期 | `let`, `&`, `&mut` | 零开销 |
| L1 | 区域（Region） | `region 'r {}`, `in 'r`, `transfer` | 零开销（批量释放） |
| L2 | 引用计数 | `Rc<T>`, `Arc<T>` | 原子操作 |
| L3 | 可选 GC | `Gc<T>`（插件式） | GC 停顿 |

## L0 所有权与借用

- `&x` / `&mut x`、`&T` / `&mut T`、`*` 解引用（G1 ✅）
- `ref` / `ref mut` 模式（`match` 臂 / `let ref x = e;` 绑定引用而非拷贝）
- 严格借用检查（Rust E0502/E0499/E0596/E0597）：`&mut` 排他、局部引用逃逸报 `DanglingReference`
- 生命周期标注 `'a`（G4 ✅，语法接受宽松检查）

## L1 区域

```rlyeh
region 'r {
    let data = BigStruct::new() in 'r;
    process(&data);
} // 批量释放
```

智能分配（`region 'r adaptive`）、精确预分配（`with_size`）、显式 bump 策略（`strategy(bump)`）—— L3 ✅ `region` 指令接线 `rlyeh-region-alloc` C ABI 运行时。

## L2 / L3 引用计数与 GC

`Box<T>`（K2）/ `Rc<T>`/`Arc<T>`（K3）/ `Gc<T>`（K4）已实现：堆分配 + `*` 解引用 + 字段/方法/索引自动剥层 + 引用计数（clone/强弱计数/弱引用/`try_unwrap`）+ 可选 GC（`gc_region` 块 + 逃逸 root + 保守标记-清除，`rlyeh-gc-runtime`）。详见 [manual/std/smart-pointers.md](./std/smart-pointers.md)。

---

[← 上一章：泛型与单态化](./08-generics.md) | [返回手册目录](./index.md) | [下一章：并发模型 →](./10-concurrency.md)
