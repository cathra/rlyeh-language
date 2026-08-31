# 9. 内存模型

> 速查四层内存模型的语法与语义。深入讲解 + 示例见 [指南 §8 内存管理](../guide/08-memory.md)。

Rlyeh 采用分层内存管理，从零开销到可选 GC 逐级递进：

| 层级 | 机制 | 关键字 | 运行时开销 |
|------|------|--------|------------|
| L0 | 静态所有权 + 借用 + 生命周期 | `let`, `&`, `&mut` | 零开销 |
| L1 | 区域（Region） | `region 'r {}`, `in 'r`, `transfer` | 零开销（批量释放） |
| L2 | 引用计数 | `Rc<T>`, `Arc<T>` | 原子操作 |
| L3 | 可选 GC | `Gc<T>`（插件式） | GC 停顿 |

> **C 程序员对照**
> 这套四层模型，本质上是把 C 程序员"凭经验选内存策略"的过程变成语言内建、编译器强制的默认行为：
> - L0 ≈ 编译期检查过的栈/值语义 + 借用（`&x` 就是 `&x`，但编译器管住"借用期间原主人不能改 / 不会 double free"）。
> - L1 ≈ 你手写的 arena 内存池（一批 `malloc`、最后 `free` 一次）。
> - L2 ≈ 你手写的 refcount 结构体（`struct { T v; int refcnt; }` + 加减引用）。
> - L3 ≈ 可选的保守 GC（如 Boehm）。
> 关键：**默认 L0 零开销，和 C 同一性能档**；安全来自编译期分析，不是运行时托管。GC 是"最后一招"。

---

## L0 所有权与借用

- `&x` / `&mut x`、`&T` / `&mut T`、`*` 解引用（G1 ✅）。
- `ref` / `ref mut` 模式（`match` 臂 / `let ref x = e;` 绑定引用而非拷贝）。
- **严格借用检查**（Rust E0502/E0499/E0596/E0597）：`&mut` 排他、活跃可变借用期间写入被借用变量报 `BorrowConflict`、`&mut` 要求 `let mut`（`BorrowMutImmutable`）、局部引用逃逸函数报 `DanglingReference`。
- 生命周期标注 `'a`（G4 ✅，语法接受、宽松检查，严格验证规划中）。

```rlyeh
let x = 10;
let r = &x;            // 不可变引用
println(*r);          // 10
let mut y = 20;
let m = &mut y;       // 可变引用
*m = 21;
```

> **语义有意宽松**：读取被借用变量与经 `*p` 写入允许（裸指针别名合法），共享借用（多个 `&`）可共存，仅直接赋值被借用变量触发冲突；`print`/`println` 参数为引用时自动剥层打印。

> **C 程序员对照**
> C 里 `int *p = &x;` 让编译器**完全不管**你之后是否 `free` 掉 `x` 所占内存后再经 `p` 访问（野指针），或同时持有可读写两份指针导致数据竞争。Rlyeh 的 `&`/`&mut` 把"借用期间原主人不能改、同一刻只能有一个可变借用"编进类型系统——编译器替你看门，而生成的机器码与 C 的取地址/解引用**同样快**（零运行时开销）。它比 Rust 严格版宽松（读取、经 `*p` 写入都允许），更贴近 C 程序员的直觉，同时仍挡住绝大多数危险写法。

---

## L1 区域

```rlyeh
region 'r {
    let data = BigStruct::new() in 'r;
    process(&data);
} // 批量释放（区域内所有对象整体回收）

fn create_data() -> BigStruct {
    region 'r {
        let data = BigStruct::new() in 'r;
        return transfer data out of 'r;   // 把所有权转出区域，区域结束时不释放它
    }
}
```

智能分配 / 显式策略（L3 ✅ `region` 指令接线 `rlyeh-region-alloc` C ABI 运行时）：

```rlyeh
region 'r adaptive { /* 自适应初始容量 */ }
region 's with_size (4096) { /* 精确预分配 */ }
region 't strategy (bump) { /* 顺序分配、整体释放，最快 */ }
```

> **bump 策略**：只维护一个指针，分配即前移——`malloc` 的 O(1) 极致版，但只能整体批量释放。区域正好满足"同生共死"场景。

> **C 程序员对照**
> `region` 就是内建版的 arena 分配器：你平时可能写 `void *pool = malloc(N); ...; free(pool);`，但 arena 里那些"子对象"各自该何时释放、有没有漏 `free` / `double free`，全靠你盯。Rlyeh 的 `region` 把"这一批同生共死"显式标出来——块结束一次性回收，逻辑上不可能泄漏 `region` 内的对象；`transfer` 则像你手写"把这个指针从 pool 里摘出来、改由调用方负责"的语义。

---

## L2 / L3 引用计数与 GC

`Box<T>`（K2）/ `Rc<T>`/`Arc<T>`（K3）/ `Gc<T>`（K4）已实现：

- 堆分配 + `*` 解引用 + 字段/方法/索引自动剥层
- 引用计数：`clone` / 强弱计数 / 弱引用 / `try_unwrap`
- 可选 GC：`gc_region` 块 + 逃逸 root + 保守标记-清除（`rlyeh-gc-runtime`）

详见 [std/smart-pointers.md](./std/smart-pointers.md)。

> **C 程序员对照**
> - `Box<T>` ≈ `T *p = malloc(sizeof(T)); ...; free(p);` 的**安全版**：离开作用域自动 `free`，且保证不会 double free / use-after-free。
> - `Rc<T>` ≈ 你手写的 `struct { T v; int refcnt; }` + `retain`/`release`；`Arc<T>` 把 `refcnt` 换成原子操作（对应 C11 `atomic_int`），跨线程安全。注意 `clone` 只是加计数、**不深拷贝数据**——这正是引用计数的意义。
> - `Gc<T>` ≈ 你接入 Boehm 这类保守 GC。默认**不开启**：绝大多数场景 L0/L1/L2 就够，且零 GC 停顿；只有遇到循环引用、或懒得手工管理生命周期时，才把对象放进 `gc_region`。

---

## 更多示例

L0 借用（严格但有意宽松）：

```rlyeh
let x = 10;
let r = &x;            // 不可变引用
println(*r);           // 10
let mut y = 20;
let m = &mut y;        // 可变引用
*m = 21;
```

> 完整讲解见 [指南 §8 内存管理](../guide/08-memory.md)。区域 / `Box`/`Rc`/`Gc` 用法见 [std/smart-pointers.md](./std/smart-pointers.md)。

> **C 程序员对照**
> 就这段而言，C 里完全等价写法是 `int x = 10; int *r = &x; printf("%d\n", *r);`——但 C 编译器不会阻止你把 `*r` 用在 `x` 已离开作用域、内存已被回收之后（未定义行为，可能恰好还能读到、也可能崩）。Rlyeh 在**编译期**保证 `r` 不比 `x` 活得久：借用检查的实质，就是把"指针不能比它指向的对象更长寿"这条 C 里靠纪律维持的规则，变成编译器强制的不变量。

---

[← 上一章：泛型与单态化](./08-generics.md) | [返回手册目录](./index.md) | [下一章：并发模型 →](./10-concurrency.md)
