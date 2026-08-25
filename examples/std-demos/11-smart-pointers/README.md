# 11 · 智能指针：Box / Rc / Arc / Gc

> 规范：docs/std-lib.md §11（智能指针）/ docs/memory-model.md（分层内存）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `Box<T>`：堆分配 + `*` 解引用 + 字段/方法/索引自动剥层 + `leak`
- `Rc<T>` / `Arc<T>`：引用计数 + `clone` + 强弱计数 + 弱引用 + `try_unwrap`
- `Gc<T>`：可选 GC（`gc_region` 块 + 逃逸 root + 保守标记-清除）

## 示例清单

| 文件 | 说明 |
|------|------|
| `box_new.zeta` | K2 Box：堆分配、解引用、自动剥层访问 |
| `box_leak.zeta` | Box::leak 泄漏为静态引用 |
| `rc_new.zeta` | K3 Rc/Arc：clone、强/弱引用计数、try_unwrap |
| `gc_region.zeta` | K4 Gc：gc_region 块 + 逃逸 root + 标记-清除回收 |

## 运行

```bash
zeta run examples/std-demos/11-smart-pointers/box_new.zeta
zeta run examples/std-demos/11-smart-pointers/box_leak.zeta
zeta run examples/std-demos/11-smart-pointers/rc_new.zeta
zeta run examples/std-demos/11-smart-pointers/gc_region.zeta
```
