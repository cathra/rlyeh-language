# 无堆编程模式（GM/T 0028 风格内存约束）

> **状态**：🟢 MVP 已落地（2026-11-15）· 编译器选项 `--no-heap`
> **定位**：为商用密码/密码模块（GM/T 0028 类安全要求）中「运行期不得进行动态内存分配」的代码提供编译器级约束与可验证的交付模式；**不替代**密码模块认证本身。

## 1. 用法

```bash
rlyeh build crypto.rl --no-heap -o crypto
rlyeh run crypto.rl --no-heap
```

`--no-heap` 隐含 `--no-std`（标准库的 `String`/`Vec`/`HashMap` 等本身依赖堆）。
测试用例可在源文件头声明：

```rlyeh
// flag: --no-heap
```

## 2. 允许的子集

| 能力 | 说明 |
|------|------|
| 标量 | `i64`/`u8`/`bool`/`f64` 等寄存器/栈槽值 |
| 定长数组 | `[T; N]` 局部数组，栈上 `[N x i64]` 分配；支持索引读写、按引用传递 |
| 栈上 struct | 任意字段数的标量 struct，局部构造、字段读写、按引用传递 |
| 函数/递归/控制流 | `fn`/`if`/`while`/`loop`/`match` 等纯计算 |
| 静态字符串 | `str` 字面量/`&str` 视图（只读数据段，不分配）；可用 `print`/`println` 内建打印标量或 `str` |
| 引用 | `&T`/`&mut T`，用于把栈聚合传给 helper（不得逃逸出栈帧） |

示例（`tests/run-pass/no_heap_crypto.rl`）：4 字状态 + 4 字 key schedule 的 64 轮混合，全部在栈上完成。

## 3. 拒绝的堆分配

| 类别 | 例子 | 诊断 |
|------|------|------|
| 堆容器 | `String` / `Vec` / `HashMap` / `HashSet` / `BTreeMap` / `VecDeque` | `NH001` |
| 智能指针 | `Box` / `Rc` / `Arc` / `Weak` / `Gc` | `NH001` |
| 宏 | `format!` / `println!` / `print!` / `eprintln!` / `dbg!` / `assert!` / `panic!` 等全部宏 | `NH001` |
| 区域分配 | `region 'r { .. }` | `NH001` |
| 内建分配/释放 | `alloc_bytes` / `alloc_array` / `array_free` | `NH001` |
| 逃逸聚合 | 数组/struct 按值跨函数传递或存入其它对象，导致 codegen 回退 `calloc` | 代码生成阶段 `no-heap` 守卫拒绝 |

`NH001` 形如：

```text
1:21: error[NH001]: no-heap mode: `Vec::new` requires heap allocation
  = help: 使用标量、定长数组或栈上 struct，避免 String/Vec/HashMap/Box/format!/region 等堆分配
```

## 4. 强制层级

1. **typecheck**：堆容器类型名/构造器、格式化宏、`region`、`alloc_*` 直接报 `NH001`；
2. **codegen 后置守卫**：扫描生成的 LLVM IR，出现 `call @calloc` / `@malloc` / `@realloc` / `@rlyeh_region_alloc` 即拒绝（覆盖「聚合逃逸 → 回退堆分配」的隐藏路径）；
3. 因此 `--no-heap` 的产物可审计为「无堆调用」，而不只是源码层声明。

## 5. 已知限制（MVP）

- **聚合按值跨函数**：`fn f(a: [i64; 4])` 或返回大 struct 可能触发逃逸→堆回退，被后置守卫拒绝；请改传 `&[i64; 4]` / `&mut Struct`（见示例），或把逻辑内联。
- **无 std**：不能用 `String`/集合/格式化；需要自行实现固定缓冲与十进制转换。
- **未提供 zeroize/constant-time 原语**：密钥清零、constant-time 比较、防侧信道仍需在 Rlyeh 代码中手写或后续标准库/内建支持；本模式只保证「无堆」。
- **未覆盖并发/actor/async/dyn/闭包**：这些机制本身可能分配；当前用例应避免，后续按需增加专门诊断。
- **认证说明**：`--no-heap` 是编译器辅助手段，不构成 GM/T 0028 认证；认证还需密钥管理、角色/服务、自检、审计、物理/环境安全等完整评估。

## 6. 验收

- `tests/run-pass/no_heap_crypto.rl`：`--no-heap` 编译运行，输出固定 4 行；
- `tests/compile-fail/no_heap_{string,vec,format,region,box}.rl`：均报 `NH001`；
- 全量套件 `rlyeh test tests` 通过。
