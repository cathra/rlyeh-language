# RFC：Rlyeh 高级 FFI 绑定（Advanced FFI Bindings）

| 字段 | 内容 |
|------|------|
| 状态 | 草案（Draft）— 待评审 |
| 日期 | 2026-09-20 |
| 范围 | 在现有「低阶 FFI」（`extern fn` 声明 + 人工 ABI 对齐）之上，补齐**高阶 FFI 绑定**能力：调用约定、链接指令、不透明类型、C 兼容布局（`repr(C)`/`union`）、函数指针与回调、变参、字符串/所有权编组、枚举 FFI 表示、`unsafe fn` 边界安全、导出到 C，以及**绑定自动生成（bindgen）**。不涉及 LLVM 后端既有调用约定生成机制的重构，仅在编译管线的「声明 → 类型检查 → codegen」链路新增 FFI 特定处理。 |
| 关联文档 | [`grammar.md`](../grammar.md) §2.3（函数/参数）、[`guide/12-ffi.md`](../guide/12-ffi.md)（低阶 FFI 指南）、[`docs/tasks/leaf/sh-p2-4-linkage-bridge.md`](../tasks/leaf/sh-p2-4-linkage-bridge.md)（链接桥接任务）、[`crates/rlyeh-actor-runtime/src/ffi.rs`](../../crates/rlyeh-actor-runtime/src/ffi.rs)（运行时 C ABI 桥接参考实现）、[`rfc/comptime.md`](comptime.md)（CTFE 与 FFI 元数据生成的交叉点） |
| 任务拆解 | 见 §11（M0–M5，建议挂 `tasks/leaf/ffi-*.md`） |

---

## 1. 背景与动机

Rlyeh 的定位是「系统级编程语言」，FFI（外部函数接口）是与 C 生态互操作的命脉：复用数十年 C 库（OpenSSL、SQLite、系统 API）、渐进式迁移（新模块用 Rlyeh、老模块留 C）、访问只有 C ABI 的底层接口。

### 1.1 当前低阶 FFI 已落地能力（事实依据）

经代码核查，0.1.0 MVP 已具备：

- **外部函数声明**：`extern fn labs(x: i64) -> i64;`（无函数体，顶层 `extern` 修饰）。codegen 生成 LLVM `declare`，由链接器解析符号（libc 提供 `labs`/`fabs` 等）。
- **`unsafe` 边界**（SH-P0-1 E3）：调用 `extern fn` 必须包在 `unsafe { }` 块内。
- **标量 + void 已验证**：`ffi_extern_test.rs` 覆盖 `i64`/`f64` 参数返回、`()`（void）返回、嵌套/循环调用、运算比较，全部通过。
- **`repr(C)` 属性已解析**：`AstStructDecl.repr_c: bool` 字段（阶段 Q1b / SH-P0-1 E2），属性语法已打通，布局接线见 `llvm_field.rs`。
- **变参语法已入规**：`grammar.md` 中 `Param ::= 'mut'? Ident ':' Type | '...'`，`...` 已作为形参。
- **`unsafe` 已是关键字**：`FnDecl ::= Attr* 'pub'? 'unsafe'? 'const'? 'async'? 'fn' ...`，`unsafe fn` / `unsafe { }` 均被解析。
- **运行时桥接范式**：`rlyeh-actor-runtime/src/ffi.rs` 展示了完整的 C ABI 互操作手写范式——`#[no_mangle] unsafe extern "C" fn` 导出、`Box::into_raw`/`Box::from_raw` 不透明句柄、`CStr` C 字符串、函数指针回调（`type RlyehHandler = unsafe extern "C" fn(...) -> u64`）、`dlsym`/`GetProcAddress` 符号解析、`#[repr(C)]` 消息结构体。

### 1.2 缺口

指南 `guide/12-ffi.md` 描述的 `extern "C" { }` 块语法与真实实现的 `extern fn` 单条声明尚不一致；且以下能力完全缺失：

1. **调用约定标注**：无法声明 `extern "system"`（Win32 stdcall）或控制 ABI。
2. **链接指令**：库名/链接方式只能靠 `Rlyeh.toml` 与示例约定，无 `#[link(name=...)]` 这类语言内指令。
3. **不透明类型**：无法声明「C 侧定义、Rlyeh 侧只持指针」的 `opaque` 类型。
4. **C 联合 / 位域**：无 `union` FFI 类型，无法映射 C `union`。
5. **函数指针一等公民**：Rlyeh 侧无法把「函数指针」作为值类型传给 C，回调场景只能靠运行时 `dlsym` 字符串约定的 workaround。
6. **变参调用**：`...` 已入规但无 codegen 与调用端实现。
7. **字符串/所有权编组**：无 `CString`/`CStr` 标准类型，无 `&str ↔ char*` 的受控转换。
8. **导出到 C**：无 `#[no_mangle]`/`#[export_name]`，Rlyeh 函数默认不被 C 调用。
9. **绑定生成**：无 `bindgen` 等价物，每个 C 库需手写 `extern fn` 与 `#[repr(C)]` 结构体。

### 1.3 典型场景

```rlyeh
// 场景 A：调用系统 C 库（libm），不透明句柄 + 回调
#[link(name = "m")]
extern "C" {
    fn hypot(x: f64, y: f64) -> f64;
    fn qsort(base: *mut c_void, n: usize, size: usize, cmp: CompareFn) -> i32;
}
type CompareFn = extern "C" fn(*const c_void, *const c_void) -> i32;

// 场景 B：映射 C 结构体（C 内存布局）
#[repr(C)]
struct sockaddr_in { sin_family: u16, sin_port: u16, sin_addr: u32 }

// 场景 C：导出 Rlyeh 函数供 C 调用
#[no_mangle]
extern "C" fn rlyeh_compute(x: i64) -> i64 { x * 2 }
```

---

## 2. 目标与非目标

**目标（Goals）**

1. 提供**声明级 FFI**：`extern "C" { }` 块、`#[link(...)]`、调用约定、`opaque` 类型、`#[repr(C)]`/`union`。
2. 让**函数指针**成为一等公民类型，使 Rlyeh ↔ C 回调对称。
3. 提供**字符串/所有权编组**原语（`CString`/`CStr`），并静态阻止 GC 类型跨越 FFI。
4. 支持**导出到 C**（`#[no_mangle]`/`#[export_name]`）。
5. 提供**绑定生成**工具（`rlyeh-bindgen` 或 `import "c" "header.h"`），降低手写成本。
6. 边界**安全可诊断**：FFI 不安全操作须有明确 `unsafe` 标注与编译期检查。

**非目标（Non-Goals）**

- 不改 LLVM 后端的调用约定生成机制（既有 C ABI 调用已工作）。
- 不做 C++ ABI（name mangling / 虚表 / 异常）——仅 C ABI。
- 不做运行时 GC 跨边界自动管理（Rlyeh GC 对象禁止直接透传 C）。
- 不实现 `extern "Rust"` 跨语言 Rust 互操作（非 C 生态）。

---

## 3. 当前能力基线（可复用资产）

| 资产 | 位置 | 状态 |
|------|------|------|
| `extern fn` 声明 → LLVM `declare` | parser `parse_fn` + codegen | ✅ 已落地 |
| `unsafe { }` 调用边界 | typecheck SH-P0-1 E3 | ✅ 已落地 |
| 标量/void 跨 FFI | `ffi_extern_test.rs` | ✅ 已验证 |
| `#[repr(C)]` 属性解析 | `AstStructDecl.repr_c` | 🔧 已解析，布局接线进行中 |
| 变参语法 `...` | `grammar.md` §2.3 | 📋 已入规，codegen 待实现 |
| `unsafe fn` 语法 | `grammar.md` §2.3 | 📋 已解析，语义待实现 |
| 运行时桥接范式 | `actor-runtime/src/ffi.rs` | ✅ 手写参考实现 |

设计原则：**尽量在既有 `extern fn` 之上增量叠加**，而非另起一套。

---

## 4. 设计总览（分层）

```
L0  声明与调用约定    extern "C" { } 块 · extern "system" · #[link] · extern fn 兼容
L1  类型与布局        opaque 类型 · #[repr(C)] / packed / align · union · 枚举 FFI repr
L2  字符串/所有权     CString/CStr · &str↔char* · Box/Arc into_raw · 禁止 GC 跨界
L3  函数指针与回调    extern "C" fn 类型 · 非捕获 fn 作回调 · Send/Sync 约束
L4  变参与导出        ... 变参调用 · #[no_mangle]/#[export_name]
L5  绑定生成          rlyeh-bindgen · import "c" header.h
L6  目标后端          native / wasm(WASI) / win32(system) 差异处理
```

---

## 5. 详细设计

### 5.1 `extern` 块与调用约定

统一三种声明形式，语义等价（均产出一个 `extern fn` 符号）：

```rlyeh
// 形式 1：既有单条声明（保留，向后兼容）
extern fn labs(x: i64) -> i64;

// 形式 2：块声明（语法糖，按块级约定展开为多条 extern fn）
extern "C" {
    fn abs(x: i64) -> i64;
    fn hypot(x: f64, y: f64) -> f64;
}

// 形式 3：内联约定
extern "system" fn MessageBoxW(...) -> i32;
```

- 调用约定取值：`"C"`（默认，平台 C ABI）、`"system"`（平台系统 ABI，Win32 为 stdcall）、`"C-unwind"`（允许跨边界 unwind）。
- 块内每条 `fn` 继承块级约定；单条可覆盖。
- codegen：在 LLVM `declare`/`define` 上附加 `attributes` 调用约定（如 `x86_stdcallcallcc`）。

### 5.2 链接指令 `#[link(...)]`

取代 `Rlyeh.toml` 的 `libs` 约定，作为语言内指令：

```rlyeh
#[link(name = "m")]            // 链接 libm（-lm）
#[link(name = "ssl", kind = "dylib")]
#[link(name = "z", kind = "static")]
#[link(name = "CoreFoundation", kind = "framework")]  // macOS framework
extern "C" { fn deflate(...) -> i32; }
```

- `kind` ∈ {`dylib`(默认) | `static` | `framework` | `raw-dylib`}。
- 编译器将指令汇总为链接器参数（`-l`/`-framework`/静态库路径），经 `driver/link.rs` 注入。
- 允许模块级 `#[link]`（作用于其后所有 `extern` 声明）与项级 `#[link]`。

### 5.3 不透明类型（Opaque Types）

C 侧定义、Rlyeh 侧仅持指针的类型：

```rlyeh
extern opaque struct sqlite3;          // 仅前向声明，Rlyeh 不知其布局
extern opaque type FILE;

fn main() {
    let db: *mut sqlite3 = open_db();  // 只能经 *mut/*const 操作
    // sizeof(sqlite3) 在 Rlyeh 侧非法（编译错误）
}
```

- 不透明类型大小/对齐在 Rlyeh 侧**未知**，禁止 `sizeof`、字段访问、值构造。
- 仅可作为 `*const T` / `*mut T` 出现，或作为 `extern fn` 指针参数/返回。
- codegen：以 `i8*`/`opaque` 指针表示，不产生布局信息。

### 5.4 C 兼容聚合布局

```rlyeh
#[repr(C)]            // 字段顺序 + C 对齐（默认按字段声明序，无重排）
struct Header { magic: u32, len: u16, flag: u8 }

#[repr(C, packed)]    // 紧凑布局，禁止内部填充（注意对齐访问安全）
struct Packed { a: u8, b: u32 }

#[repr(C, align(16))] // 整体对齐到 16 字节
struct Aligned { data: [u8; 8] }

extern union sockaddr {  // C 联合：所有字段共享存储，大小为最大字段
    sa: sockaddr_in,
    raw: [u8; 16],
}
```

- `#[repr(C)]`：字段按声明顺序，遵循目标 C 对齐规则（与 clang 一致）。
- `#[repr(packed)]`：移除内部填充；访问未对齐字段在 `unsafe` 下允许（提示风险）。
- `#[repr(align(N))]`：整体对齐上界。
- `union`：Rlyeh FFI 专用聚合，字段重叠；读写需 `unsafe`（无主动态标签），语义对齐 C `union`。
- 布局算法需与 `driver` 的目标 triple（`host_triple`/`target_arch`/`target_os_code`）联动。

### 5.5 函数指针与回调

把「函数指针」提升为一等公民类型：

```rlyeh
type CompareFn = extern "C" fn(*const c_void, *const c_void) -> i32;

extern "C" {
    fn qsort(base: *mut c_void, n: usize, size: usize, cmp: CompareFn) -> i32;
}

// Rlyeh 侧提供回调（非捕获 fn 项，等价 Rust 的 fn 指针）
extern "C" fn my_cmp(a: *const c_void, b: *const c_void) -> i32 { /* ... */ }

fn main() {
    unsafe { qsort(arr, n, size, my_cmp); }   // 传 Rlyeh 函数指针给 C
}
```

- `extern "C" fn` 类型与裸指针同属 FFI 安全类型，可作参数/返回/字段。
- **仅非捕获 `fn` 项与 `static` 闭包**可作 `extern "C" fn` 值（捕获闭包无法跨 FFI）。
- 作为回调传给 C 时，要求该 `fn` 满足 `Send`（单线程运行时可放宽）。
- 反向：C 回调进 Rlyeh，经 `extern "C" fn` 接收（即 actor 运行时 `RlyehHandler` 的通用化）。

### 5.6 变参函数

`...` 已入规，补齐调用端与 codegen：

```rlyeh
extern "C" fn printf(fmt: *const u8, ...) -> i32;

fn main() {
    unsafe {
        let s = CString::from("hello %d\n");
        printf(s.as_ptr(), 42);   // 调用端按 C varargs 传参
    }
}
```

- 变参仅允许出现在 `extern` 函数（Rlyeh 自有函数不支持变参）。
- codegen：目标平台 C varargs（LLVM `va_arg` / 平台寄存器约定），调用端用 `...` 实参逐个落地。
- 调用须在 `unsafe`（变参类型校验无法静态保证）。

### 5.7 字符串与所有权编组

新增标准库类型（见 `std-lib.md` FFI 模块规划）：

```rlyeh
// 自有堆、以 NUL 结尾、可安全交出裸指针
let c = CString::from("hello");     // Rlyeh String -> CString
let p: *const u8 = c.as_ptr();      // 借用裸指针（生命周期随 c）
// 跨越 FFI 时保证 p 在使用期间有效

// C 返回的字符串（Rlyeh 不拥有，禁止 free）
let borrowed: CStr = CStr::from_ptr(c_ptr);
let s: String = borrowed.to_string();  // 拷贝回 Rlyeh 拥有

// 所有权转移：Rlyeh 堆对象交给 C 长期持有
let raw = Box::into_raw(my_struct);    // *mut T
extern_free(raw);                     // C 侧负责释放（须用同一分配器）

// 区域分配内存亦可交 C（region 释放语义需显式 transfer/释放）
```

- `&str ↔ char*`：提供受控转换，`unsafe` 下使用，编译器提示「C 侧不得超出借用期持有」。
- **禁止 GC 类型跨界**：`Gc<T>`/受托管值在 FFI 边界属类型错误（静态诊断），因 C 无法参与 GC。
- `Arc<T>`/`Rc<T>` 跨界须用 `into_raw`/`from_raw` 显式桥接（引用计数由 Rlyeh 侧维护）。

### 5.8 枚举 FFI 表示

```rlyeh
#[repr(C)]          // 字段无关枚举：以 C int（i32）为后端
enum Color { Red, Green, Blue }   // C 侧看作 int 0/1/2

#[repr(u8)]         // 显式底层类型
enum Mode: u8 { Read = 1, Write = 2 }
```

- 字段无关 `enum` 默认 `#[repr(C)]` 映射为 `i32`（与 C `enum` 兼容）。
- 带底层类型 `#[repr(u8/u16/u32/...)]` 控制宽度；C 侧 `#[repr(i32)]` 亦支持。
- 域枚举（带负载）跨 FFI 需 `#[repr(C)]` 且负载为 FFI 安全类型（见 §7）。

### 5.9 `unsafe fn` 与边界安全

- `unsafe fn` 声明（`grammar.md` 已支持）调用时**无需**再包 `unsafe { }`，但调用点仍计入 unsafe 上下文审计。
- 既有 `unsafe { extern_call() }` 块形式保留（向后兼容 SH-P0-1 E3）。
- 引入「FFI 安全类型」概念（§7），`extern fn` 签名若含非安全类型（如 `String`、含 GC 的类型）编译报错。

### 5.10 导出到 C

```rlyeh
#[no_mangle]                 // 保留符号名 rlyeh_compute
extern "C" fn rlyeh_compute(x: i64) -> i64 { x * 2 }

#[export_name = "my_entry"] // 自定义导出名
extern "C" fn entry() { }
```

- codegen：函数加 `attributes #0 = { noinline nounwind }` 与 `unnamed_addr` 控制，并**导出符号**（不 internal）。
- 与现有 actor 运行时 `#[no_mangle] unsafe extern "C" fn rlyeh_actor_*` 导出机制对齐（Rust 侧已验证）。

### 5.11 绑定生成（bindgen）

两种形态，择一或并存：

1. **声明式 `import`**：`import "c" "sqlite3.h"` — 编译器内建轻量 C 头解析器，生成 `extern fn` + `opaque` + `#[repr(C)]` 结构 + 常量宏。
2. **独立工具 `rlyeh-bindgen`**：读取 C 头，输出 `.rl` 绑定文件（可纳入版本控制、可审查）。

- 处理：函数签名、`typedef`、结构体/`union`（递归布局）、枚举（repr）、函数指针 typedef、宏（降级为 `const`）、`#define` 常量、内联函数（跳过或 `@inline` 近似）。
- 不支持：C++ 特性、复杂宏展开（报「需手写绑定」提示）。

---

## 6. 类型映射表（ABI 对齐）

| Rlyeh 类型 | C 类型 | 备注 |
|------------|--------|------|
| `i8`/`i16`/`i32`/`i64` | `int8/16/32/64_t` | 直接对应 |
| `u8`..`u64` | `uint8/16/32/64_t` | 直接对应 |
| `f32`/`f64` | `float`/`double` | 直接对应 |
| `bool` | `_Bool`/`uint8_t` | 注意 C `int` 不可当 bool |
| `char` | `uint32_t` | Rlyeh char 是 32 位码点，与 C `char`(8位) 不同 |
| `*const T`/`*mut T` | `const T*`/`T*` | 裸指针互视（G3 ✅） |
| `extern opaque T` | 前向声明 struct | 仅 `*const/*mut` |
| `#[repr(C)] struct` | C struct | 字段序+对齐一致 |
| `union` | C union | 共享存储 |
| `extern "C" fn(...)` | 函数指针 | 一等公民 |
| `CString`/`CStr` | `char*` | 编组原语 |
| 枚举 `#[repr(C)]` | C `enum`/`int` | i32 后端 |
| `()` | `void` | 无返回 |
| ❌ `String`/`Vec`/`Gc<T>`/捕获闭包 | — | 禁止直接跨界（须编组/`into_raw`） |

---

## 7. 安全模型与诊断

- **FFI 不安全类型清单**：`String`、`Vec`、`HashMap`、含 `Gc<T>` 的聚合、捕获闭包。出现在 `extern fn` 签名或 `extern "C" fn` 类型中 → 编译错误，提示「请经 `CString`/`into_raw`/不透明指针编组」。
- **`unsafe` 审计**：`extern fn` 调用、`union` 字段访问、变参调用、裸指针解引用、手工 `into_raw`/`from_raw` 均须处于 `unsafe` 上下文。
- **回调 `Send` 约束**：`extern "C" fn` 作为值传给 C 并可能被跨线程回调时，要求其为 `Send`（单线程/actor 运行时可放宽）。
- **跨边界 unwind**：默认 `#[repr(C)]` 函数遇到 Rlyeh panic 须 `abort`（不可 unwind 进 C）；`extern "C-unwind"` 显式允许（平台支持时）。
- **生命周期提示**：`&str`/`&T` 借给 C 时，诊断建议「确保 C 侧不在借用期外使用」。

---

## 8. 代码生成要点（LLVM IR）

- `extern fn` → `declare <ret> @name(<params>)`（无函数体），按约定附加 calling conv 属性。
- Rlyeh 导出 → `define` + `attributes`(`nounwind`) + 符号**导出**（非 internal），调用约定与 C 侧匹配。
- `union` → LLVM `<{ }>` 或 `iN` 重叠存储（`{ i32, [4 x i8] }` 最大成员）。
- `#[repr(C)]` 结构 → 按目标对齐生成 `{ ty1, ty2, ... }`，与 clang 输出布局一致（用 `llvm_field.rs` 已有的字段布局逻辑）。
- 变参 → 平台 C varargs 调用序列（x86-64 System V / Win64 / AAPCS）。
- 不透明类型 → `i8*`（或 target 指针宽度）。

---

## 9. 目标后端差异

| 目标 | FFI 处理 |
|------|----------|
| native（macOS/Linux/Windows） | `clang`/`ld` 链接，符号直接解析；Win32 用 `"system"`（stdcall） |
| wasm / WASI | 当前 actor 桥接已用静态符号表 `rlyeh_actor_resolve` 替代 `dlsym`（`link.rs` L4b）；FFI 到 JS 走 `extern "wasm"` import/export + 宿主注入 JSON（wasm-bindgen 风格），与 C FFI 为独立轴 |
| 交叉编译 | `target_os_code`/`target_arch` 决定布局与约定（如 macOS `sockaddr_in` 含 `sin_len`） |

---

## 10. 测试与示例

- **单元/集成测试**：扩展 `crates/rlyeh-driver/tests/ffi_extern_test.rs`：
  - 标量/void（已覆盖）→ 扩展字符串（`CString`）、`#[repr(C)]` 结构布局对拍（与 C 端 `sizeof`/`offsetof` 比对）、不透明句柄往返、`extern "C" fn` 回调（qsort 式）、变参（`printf`）、`#[no_mangle]` 导出被 C `main` 调用。
- **混合构建示例**：`examples/ffi/` 下 `+`.c` + `.rl`，验证 `rlyeh build main.rl foo.c`。
- **绑定生成**：`rlyeh-bindgen` 对真实小头文件（如 `zlib.h` 子集）生成 `.rl` 并编译通过。

---

## 11. 路线图（M0–M5，建议任务拆解）

| 里程碑 | 内容 | 依赖 |
|--------|------|------|
| **M0** 调用约定与链接 | `extern "C" { }` 块语法糖、`extern "system"`、`#[link(name,kind)]` 解析与注入 `link.rs` | `extern fn` 已有 |
| **M1** 不透明与布局 | `extern opaque` 类型、`#[repr(C/packed/align)]` 布局接线、`union` 类型 | M0 |
| **M2** 函数指针与回调 | `extern "C" fn` 一等公民类型、非捕获 `fn` 作回调、Send 约束 | M0 |
| **M3** 编组与变参 | `CString`/`CStr` 标准类型、`&str↔char*`、`into_raw`/`from_raw`、变参 `...` 调用+codegen | M1, M2 |
| **M4** 导出与安全 | `#[no_mangle]`/`#[export_name]`、枚举 FFI repr、`unsafe fn` 语义、FFI 安全类型静态诊断 | M0 |
| **M5** 绑定生成 | `rlyeh-bindgen` / `import "c"` 头解析，覆盖函数/struct/union/enum/函数指针/常量 | M1, M3 |

建议挂 `docs/tasks/leaf/ffi-m0-linkage.md` … `ffi-m5-bindgen.md`，并在 `development-plan.md` §6.3c（U–Z）消解。

---

## 12. 开放问题 / 风险

1. **`extern "C" { }` 与既有 `extern fn` 的归一**：建议块语法为语法糖，typecheck 内部统一为 `extern fn` 条目，避免两套表示分裂。
2. **`repr(C)` 布局与 clang 的完全对齐**：需对拍测试（尤其 bitfield、union、对齐填充），可能需复用/扩展 `llvm_field.rs`。
3. **变参调用的类型安全**：C varargs 本质不安全，是否限制「变参实参必须 FFI 安全标量/指针」以降风险？
4. **WASM FFI 与 C FFI 的分流**：WASM 目标下 `extern "C"` 应映射到 wasm import 还是仍走宿主 C 链接？建议在 `link.rs` 按 target 分支。
5. **bindgen 头解析范围**：首版是否仅支持纯 C（`std=` 子集），C++ 显式报错？
6. **捕获闭包跨界**：明确禁止（与 Rust 一致），或支持 `static` 闭包？建议首版仅 `fn` 项。

---

## 13. 附录：与现有 `extern fn` 的兼容性

- 既有 `extern fn labs(x: i64) -> i64;` 与 `unsafe { labs(-42) }` **完全保留**，不作为破坏变更。
- `extern "C" { fn abs(x: i64) -> i64; }` 仅是前者的分组写法；二者在 `AstItem`/`typecheck` 内部归一为同一 `extern fn` 表示。
- `#[repr(C)]` 属性已解析，M1 仅补布局接线，无语法破坏。
- actor 运行时的手写桥接（`ffi.rs`）可作为 M2/M4 的「函数指针回调」「`#[no_mangle]` 导出」的参考实现与回归基准。
