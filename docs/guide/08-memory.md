# 8. 内存管理：分层所有权模型

Rlyeh 采用分层内存管理，从零开销到可选 GC 逐级递进：

| 层级 | 机制 | 关键字 | 运行时开销 |
|------|------|--------|------------|
| L0 | 静态所有权 + 借用 + 生命周期 | `let`, `&`, `&mut` | 零开销 |
| L1 | 区域（Region） | `region 'r {}`, `in 'r`, `transfer` | 零开销（批量释放） |
| L2 | 引用计数 | `Rc<T>`, `Arc<T>` | 原子操作 |
| L3 | 可选 GC | `Gc<T>`（插件式） | GC 停顿 |

## 8.1 引用与借用（L0）

`&x` / `&mut x` 表达式、`&T` / `&mut T` 参数与返回、解引用 `*` 已实现（G1 ✅）；`ref` / `ref mut` 模式（`match` 臂与 `let ref x = e;` 绑定引用而非拷贝）已实现。

```rlyeh
let x = 10;
let r = &x;                // 不可变引用
println(*r);              // 10
let mut y = 20;
let m = &mut y;           // 可变引用
*m = 21;
```

**严格借用检查已实现**（Rust E0502/E0499/E0596/E0597 对应）：`&mut` 与任何活跃借用互斥、多个 `&mut` 互斥、活跃可变借用期间写入被借用变量报 `BorrowConflict`；`&mut` 要求 `let mut` 绑定（`BorrowMutImmutable`）；局部引用逃逸函数（尾表达式 / `return` 返回 `&x`）报 `DanglingReference`。

## 8.2 区域系统（L1）

```rlyeh
region 'r {
    let data = BigStruct::new() in 'r;
    process(&data);
} // 批量释放

fn create_data() -> BigStruct {
    region 'r {
        let data = BigStruct::new() in 'r;
        return transfer data out of 'r;
    }
}
```

智能分配（PGO 画像回灌初始容量）、精确预分配 / 显式 bump 策略（L3 ✅：`region` 指令接线 `rlyeh-region-alloc` C ABI 运行时）：

```rlyeh
region 'r adaptive {
    for i in 0..<10000 {
        let obj = Data::new(i) in 'r;
    }
}
region 's with_size (4096) {
    let buf = BigStruct::new() in 's;
}
region 't strategy (bump) {
    let p = Point { x: 1, y: 2 } in 't;
}
```

## 8.3 引用计数与 GC（L2 / L3）

`Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>` 已实现（K2/K3/K4）：堆分配 + `*` 解引用 + 字段/方法/索引自动剥层 + 引用计数（clone/强弱计数/弱引用/`try_unwrap`）+ 可选 GC（`gc_region` 块 + 逃逸 root + 保守标记-清除，`rlyeh-gc-runtime`）。

---

[← 上一章：模块系统](./07-modules.md) | [返回指南目录](./index.md) | [下一章：Actor 并发 →](./09-actors.md)
