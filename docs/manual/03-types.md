# 3. 类型系统

> 本章是**类型速查 + 语义说明**。每个类型给出形态、字节宽、与 C 的对应、以及最小示例。深入讲解见 [指南 §5 聚合类型](../guide/05-aggregates-generics.md)、[§8 内存](../guide/08-memory.md)。

---

## 3.1 值类型（标量）

| 类型 | 说明 | 字节 | C 对应 | 备注 |
|------|------|------|--------|------|
| `i8` | 有符号 8 位 | 1 | `int8_t` | |
| `i16` | 有符号 16 位 | 2 | `int16_t` | |
| `i32` | 有符号 32 位 | 4 | `int32_t` | |
| `i64`（默认整数） | 有符号 64 位 | 8 | `int64_t` | `let x = 6;` 推断为 `i64` |
| `isize` | 指针宽有符号 | 平台 | `intptr_t` | 与 `i64` **不相交**，不能互作案联合成员 |
| `u8` | 无符号 8 位 | 1 | `uint8_t` | |
| `u16`/`u32`/`u64`/`usize` | 无符号 | — | 对应 `uintN_t` | |
| `f32` | 32 位浮点 | 4 | `float` | |
| `f64`（默认浮点） | 64 位浮点 | 8 | `double` | `let y = 3.14;` 推断为 `f64` |
| `bool` | 布尔 | 1 | `_Bool` | `true`/`false`；非零整数 `as bool` 为真 |
| `char` | **32 位 Unicode 码点** | 4 | 无直接对应（C `char` 仅 8 位） | `'a' as i64` = 97 |
| `()` | 单元类型 | 0 | `void` | 空返回 / 占位 |

> **C 对照要点**：
> 1. Rlyeh 整数**默认 `i64`**，浮点**默认 `f64`**——和 C 里 `6` 是 `int`、`3.14` 是 `double` 类似，但 Rlyeh 没有"默认 int 是 32 位还是 64 位"的平台差异（明确 64 位）。
> 2. **`char` 是 32 位**，不是 C 的 8 位 `char`。处理 ASCII 字节流请用 `u8`。
> 3. **`isize` 与 `i64` 不相交**（类型联合 `i64 | isize` 会报错）——它们虽然运行时都是 64 位（64 位平台），但类型系统视为不同种类。

### 示例

```rlyeh
let a: i8 = -1;
let b: u8 = 255;
let big = 6;            // 推断 i64
let pi = 3.14;          // 推断 f64
let flag = true;        // bool
let c = 'A';            // char（码点 65）
let unit = ();          // ()
```

---

## 3.2 聚合类型

| 类型 | 说明 | 示例 |
|------|------|------|
| `struct { ... }` | 具名字段聚合 | `struct Point { x: f64, y: f64 }`；字面量 `Point { x: 1, y: 2 }` |
| `[T; N]` | 定长数组（N 编译期已知） | `[1, 2, 3]` / `[i64; 4]` |
| `Vec<T>` | 动态数组（标准库） | `Vec::new()` / `vec![1, 2, 3]` |
| `String` | UTF-8 字符串缓冲（标准库） | `String::from("hi")` |
| `Str` | **未实现**，勿用 | — |
| `&T` / `&mut T` | 引用（G1 ✅） | `&x` / `&mut x`；`*` 解引用；字段/方法自动剥引用层 |
| `&str` | String 只读借用视图 | `s.as_str()`（零拷贝） |
| `&[T]` / `&mut [T]` | 切片引用（零拷贝胖指针 `{data, len}`） | `&arr` 后 `xs[1..<3]`；`.len()`/`.first()`/`.last()`/`.iter()`；`&[T; N]` 经 unsize coercion → `&[T]` |
| `*const T` / `*mut T` | 裸指针（与 `&T` 互视，G3 ✅） | `let p: *const i64 = &x;` |
| `Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>` | 智能指针 | 见 §9 / [std/smart-pointers](./std/smart-pointers.md) |
| `Option<T>` / `Result<T, E>` | 代数数据类型（标准库） | `Some(5)` / `Result::Ok(10)` |
| `dyn Protocol` | protocol 对象（2 槽胖指针，H4 ✅） | `let d: dyn Shape = &c;` |
| `fn(T) -> R` | 函数指针类型（H1 ✅） | `let f: fn(i64) -> i64 = add;` |
| `A | B` | 类型联合（U1/U2 ✅） | `let x: i64 | String = 5;` |
| `enum { ... }` | 具名变体枚举；可带显式判别式（U3 ✅） | `enum Code { Ok = 200 }` |

> **C 对照**：
> - `struct` 默认**值语义**（拷贝即深拷贝），类似 C 的 `struct` 赋值；但 Rlyeh 的 `struct` 可以有 `impl` 方法。
> - `[T; N]` ≈ C 数组 `T arr[N]`（长度在类型里），但 Rlyeh 赋值是整体拷贝、且不退化成指针。
> - `&T` ≈ C 的 `T*`（取地址），但带借用检查。
> - `*const T` ≈ C 的 `const T*`，且可与 `&T` 互视（宽松）。

---

## 3.3 类型推断

`let` 绑定默认推断：整数字面量 → `i64`，浮点字面量 → `f64`。

- **函数参数 / 返回须显式注解**：`fn add(a: i64, b: i64) -> i64`。
- **闭包参数**：有注解用注解；无注解由**首次调用点实参推断**（延迟固化，见 [指南 §3.2 H5](../guide/03-basic-syntax.md)）。

```rlyeh
let x = 6;                 // x: i64（推断）
let y = 3.14;              // y: f64（推断）
let mut v = Vec::new();    // v: Vec<_>（元素类型由首次 push 推断）
v.push(1);                 // 现推断为 Vec<i64>
```

---

## 3.4 类型转换

- **隐式**：字面量按目标类型 reinterpret 64 位槽（跨整族/浮点数值不变，N1）；ASCII 码点转义；字符串字面量自动升级为 `String`。
- **`as` 显式（U6 ✅）**：数值族 ↔ 之间——向零截断 / 环绕截断；同类型零指令。

```rlyeh
let a = 3.7 as i64;        // 3（向零截断）
let b = 300 as i8;         // 44（截断环绕）
let d = 5 as bool;         // true（非零即真）
let m = 'a' as i64;        // 97（码点 → 整数）
```

> 范围：≤64 位整族 + 浮点 + bool + char（32 位码点）。`i128`/`u128` 与指针/引用/聚合转换保持擦除。

---

## 3.5 引用与借用

- `&x` / `&mut x` 表达式、`&T` / `&mut T` 参数与返回、`*` 解引用（G1 ✅）。
- `&str` 只读视图（G2 ✅）；`str` 值一等类型（字面量/绑定字面量变量自动升级为 String）。
- 生命周期标注 `'a`（G4 ✅，语法接受、宽松检查，严格验证规划中）。
- **严格借用检查**（Rust E0502/E0499/E0596/E0597）：`&mut` 排他、局部引用逃逸报 `DanglingReference`。

```rlyeh
let x = 10;
let r = &x;            // &i64
println(*r);          // 10
let mut y = 20;
let m = &mut y;       // &mut i64
*m = 21;
```

---

## 3.6 智能指针

| 类型 | 机制 | 状态 | 用途 |
|------|------|------|------|
| `Box<T>` | 独占堆分配 | K2 ✅ | 单一 owner 的堆对象 |
| `Rc<T>` | 引用计数（单线程） | K3 ✅ | 多处共享（非线程） |
| `Arc<T>` | 原子引用计数（多线程） | K3 ✅ | 跨线程共享 |
| `Gc<T>` | 可选 GC（保守标记-清除） | K4 ✅ | 图结构/循环引用 |

详见 [std/smart-pointers.md](./std/smart-pointers.md) 与 [指南 §8.5/§8.6](../guide/08-memory.md)。

---

## 3.7 字符串类型

| 类型 | 形态 | 说明 |
|------|------|------|
| `String` | 堆缓冲（data/len/cap 三槽） | O(n) 扩容，可增长，离开作用域自动释放 |
| `str`（字面量值） | 指向静态数据的 `i8*` | 参与操作自动升级为 `String` |
| `&str` | 只读借用视图 | `len()` / `r[i]` / `substring`，零拷贝 |

> **C 对照**：`String` ≈ 你手写的 `struct { char* data; size_t len; size_t cap; }` + 自动 `realloc` + 自动 `free`。`&str` ≈ `const char*`（但带长度，不是 `\0` 结尾依赖）。

---

## 3.8 类型联合 `A | B`（U1 / U2 ✅）

`A | B` 表示「值为 A 或 B」，运行时为匿名 enum（tag + payload）。

- 成员须**两两互不相交**：`i64 | isize`、`&i64 | &mut i64` 报 `UnionMembersNotDisjoint`。
- `match` 用**类型臂**解构：`i64 => ...`、`String => ...`（payload 绑定到同名变量）。
- 未收窄的联合禁止直接运算 / 方法调用（`compatible_with` 单向：成员 → 联合）。
- 优先级：`&T | &mut U` = `(&T) | (&mut U)`。

```rlyeh
let x: i64 | String = 5;
match x {
    i64 => println(i64),
    String => println(String.len()),
}
```

---

## 3.9 枚举显式判别式（U3 ✅）

```rlyeh
enum Code { Ok = 200, NotFound = 404, Error = 500 }
let c = Code::Ok;        // tag = 200
```

判别值即该变体的 tag，构造与 `match` 均复用。等价于 C 的 `enum Code { Ok = 200, ... };`。

---

## 更多示例

类型推断与显式注解、裸指针互视：

```rlyeh
let x = 6;              // x: i64（推断）
let y = 3.14;           // y: f64（推断）
let v = Vec::new();
v.push(1);              // 推断为 Vec<i64>
let p: *const i64 = &x; // 裸指针（与 &T 互视，G3 ✅）
```

可运行版本见 [`examples/by-chapter/03-basic-syntax.rl`](../../examples/by-chapter/03-basic-syntax.rl)。

---

[← 上一章：词法与字面量](./02-lexical.md) | [返回手册目录](./index.md) | [下一章：表达式与运算符 →](./04-expressions-operators.md)
