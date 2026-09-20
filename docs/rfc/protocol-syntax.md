# RFC：Protocol 语法（`protocol` / `impl T: P` 取代 `protocol` / `impl Protocol for Type`）

| 字段 | 内容 |
|------|------|
| 状态 | Draft（4 项开放问题已于 2026-09-18 决议；PC-0 / PC-1 已落地；**同日二次复议 PC-8：扩展块语法改为 `impl T: P` / `impl T`**（§7.7）；**PC-7 活文档/技能全量改写已落地**（§7.8）；**PC-9 多协议 `impl T: A, B` 已落地**（§7.9）；**PC-10 `dyn A → dyn B` 静态上转已落地**（§7.10）；**PC-11 `protocol` / `extension` 关键字已彻底移除**（§7.11）；**PC-12 旧语序 `impl P for T` 已移除、语法收敛为 `protocol` + `impl T: P` / `impl T`**（§7.12）；其余待评审；评审通过后再并入 `grammar.md` / `semantics.md`） |
| 日期 | 2026-09-18 |
| 范围 | 语言**前端语法**（词法 / 语法 / 名字解析 / 格式化 / 工具链与文档迁移）；**不改变**类型系统的单态化、`dyn` 分派、vtable 布局等运行时/语义模型 |
| 关联文档 | [`grammar.md`](../grammar.md) §2.6 / §2.7、[`semantics.md`](../semantics.md)、[`guide/05-aggregates-generics.md`](../guide/05-aggregates-generics.md)、[`manual/08-generics.md`](../manual/08-generics.md)、[`CODEBUDDY.md`](../../CODEBUDDY.md) §3.5.5、[`rfc/borrow-simplification.md`](./borrow-simplification.md)（同属"去 Rust 味"演进线） |
| 任务拆解 | 见 §9 |

---

## 1. 背景与动机

Rlyeh 的抽象机制沿用了 Rust 的 `protocol` + `impl ... for ...` 模型。其**语义内核**（方法集约束、单态化、`dyn` 分派、关联类型、默认方法）是正确且必要的，但**语法形态**带有两个明显的"Rust 味"负担：

1. **`protocol` 一词的认知负荷**：`protocol` 是 Rust 特有术语（源自 Scala），在主流语言（Swift `protocol` / Java·C# `interface` / Go `interface` / C++ `concept`）中，同类概念几乎都叫"协议/接口"。用户读到 `protocol` 时会不自觉代入 Rust 的所有权/`dyn`/对象安全等一整套语境。

2. **`impl <Protocol> for <Type>` 违反主流"声明点一致性"习惯**：
   - **一致性信息被藏在别处**：类型是否满足某协议，必须**逐个浏览全文件的 `impl` 块**才能确认；声明 `struct Shape { ... }` 处对"Shape 能做什么"零信息。
   - **写法与主流 OO / 协议式语言相反**：Swift `struct C: P`、Java `class C implements P`、C# `class C : P` 都把**协议写在类型声明上**；`impl P for C` 是 Rust 独有语序（且 `for` 一词与 `for` 循环关键字重载）。
   - **固有方法与协议实现视觉难区分**：`impl Foo { }`（固有）与 `impl Protocol for Foo { }`（协议）只差一个 `for` 子句，阅读/搜索时极易混淆。

**核心主张**：用 Swift 风格的 `protocol` + **声明点一致性**（`struct C: P`）替换 `protocol` + `impl P for C`；用 **`impl T: P` / `impl T`**（Swift 式语序，`impl` 关键字保留）承担**追溯式一致性**与**固有方法**。Rlyeh 现有的 `:` 已经承载"约束/归属"语义（`T: Bound`、字段 `name: Type`、`where T: Bound`），把 `C: P` 复用为"类型 C 归属于协议 P"，**心智一致、零新增符号**。

### 1.1 Before / After（一眼对比）

```rlyeh
// ── 现状（Rust 味）─────────────────────────────
protocol Area {
    fn area(&self) -> f64;
}

impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle(r) => 3.14 * r * r,
            Shape::Rect { w, h } => w * h,
        }
    }
}
```

```rlyeh
// ── 方案（Swift 味）─────────────────────────────
protocol Area {
    fn area(&self) -> f64;
}

// 一致性写在类型声明上；协议成员可直接内联（也可放 impl 块）
struct Shape: Area {
    // …… 若 Shape 为 enum，则此处为 variant 列表，方法同样可内联
}
```

其中 `impl Area for Shape { ... }` 等价改写为：

```rlyeh
impl Shape: Area {
    fn area(&self) -> f64 { /* ... */ }
}
```

---

## 2. 目标与非目标

### 2.1 目标

- **G1**：`protocol` 成为声明"行为契约"的唯一关键字；`protocol` 关键字已移除（PC-11，见 §7.11）。
- **G2**：一致性**默认写在类型声明点**（`struct C: P`、`enum E: P`），让"这个类型能做什么"在声明处即可见。
- **G3**：`impl T: P` 提供追溯式一致性（`impl C: P { }`）与固有方法（`impl C { }`），覆盖跨文件 / 后置扩展（`impl` 关键字保留，旧语序 `impl P for T` 退役）。
- **G4**：保留全部既有能力——默认方法、关联类型、泛型协议与约束（`T: P`、`where`）、`Self`（返回位）、`dyn` 分派——**语义零变更**。
- **G5**：顺带落地 grammer 中已规划但**未实现**的**协议继承** `protocol A: B`。
- **G6**：提供**无破坏迁移路径**：解析器过渡期双写接受旧语法 + 提供自动改写工具。

### 2.2 非目标

- 不改变单态化 / `dyn` vtable / 对象安全 / `Self` 返回位限制等运行时与类型语义。
- 不引入 Swift 的 `associatedtype` 关键字（**已决议**：沿用既有 `type Item;`，见 §3.6）。
- 不引入 Swift 的 `any` 关键字（**已决议**：保留 `dyn P`）。
- 不合并 `enum`/`struct` 为 `type`（本 RFC 只动"协议与一致性"）。
- 不改变 `#[derive(...)]` 机制（派生仍生成一致性代码，见 §7.4）。

---

## 3. 核心设计

### 3.1 关键字：`protocol` 取代 `protocol`

```rlyeh
protocol Draw {
    fn draw(&self);
}
```

- `protocol` 为**保留字**；原 `protocol` 关键字**已从语法中移除**（PC-11，见 §7.11）。
- 语法位置与 `protocol` 完全一致：`pub`? 可修饰、泛型参数、`where`、`:`（协议继承）、成员体。

### 3.2 声明点一致性：`Type: ProtocolList`

一致性列表写在类型名之后、泛型/`where` 之前：

```rlyeh
struct Circle: Draw {
    radius: f64,
    // 协议成员可直接内联（Swift 风格）；也可只声明字段、成员放 impl 块
    fn draw(&self) { println(self.radius); }
}

enum Shape: Draw, Area {     // 多个协议：逗号分隔
    Circle(f64),
    Rect { w: f64, h: f64 },
}

pub struct Widget: Draw, Debug, Clone {   // 与 pub 组合
    id: i64,
}
```

语法位置（与既有 `RegionParam? GenParams?` 顺序衔接）：

```
'struct'  Ident RegionParam? GenParams? (':' ProtocolList)? WhereClause? '{' StructMember* '}'
'enum'    Ident RegionParam? GenParams? (':' ProtocolList)? WhereClause? '{' EnumBody '}'
```

> **为何用 `:` 而非 `implements`/`for`**：Rlyeh 的 `:` 已是"归属/约束"统一符号（字段 `x: T`、约束 `T: Bound`、`where T: Bound`），`C: P` 自然读作"C 归属于 P（C 满足协议 P）"，且**无新增关键字**、与 Swift 一致。

### 3.3 类型体内联成员（Swift 风格）

为使"类型能做什么"自包含，`struct`/`enum` 体除字段/variant 外，允许内联协议成员与固有成员：

```rlyeh
struct Counter: Draw {
    value: i64,                       // 字段

    fn draw(&self) { println(self.value); }   // 协议成员（内联）
    fn bump(&mut self) { self.value = self.value + 1; }  // 固有成员（内联）

    fn new() -> Counter { Counter { value: 0 } }         // 关联构造
}
```

**裁决规则**（避免"内联成员到底属于哪个协议"的歧义）：内联成员**先视为类型的固有成员**；类型声明的一致性列表 `: P` 只声明"该类型满足 P"。若某内联成员与 `P` 的必需成员**同名同签名**，则它**即 P 的实现**；其余为固有成员。该匹配在 `desugar` 阶段借助已收集的协议定义完成（§6），无需在 parser 阶段知道 `P` 的内容。

> 备选已否决：要求内联成员必须带协议限定（`fn P::area(..)`）——过于冗长，且违背 Swift 习惯。同名匹配 + 冲突诊断（两协议要求同名不同签名）足够。

### 3.4 `impl T: P` / `impl T`：追溯式一致性与固有方法

沿用 `impl` 关键字，但采用 **Swift 式语序**（类型在前、协议在后），承担三类场景：

```rlyeh
// ① 追溯式一致性：为已有类型补一个协议（跨文件 / 第三方类型）
impl Point: Draw {
    fn draw(&self) { println(self.x); }
}

// ② 固有方法（无协议）：等价于旧 `impl Point { ... }`
impl Point {
    fn norm2(&self) -> f64 { self.x * self.x + self.y * self.y }
    fn new(x: f64, y: f64) -> Point { Point { x: x, y: y } }
}

// ③ 泛型类型（泛型既可写在 impl 前缀，也可写在类型上；推荐后者，Swift 风格）
impl<T> Pair<T>: Wrap<T> {
    fn wrap(&self) -> i64 { 0 }
}
impl Pair<T>: Wrap<T> {                 // 等价写法（泛型由类型引入）
    fn wrap(&self) -> i64 { 0 }
}

// ④ impl 块级约束（where）原样保留
impl<T> Pair<T>: Wrap<T> where T: Speak {
    fn wrap(&self) -> i64 { self.a.speak() }
}
```

- `impl` 前缀的 `GenParams?` 为可选；若同时写在类型上，须一致（推荐只写在类型上）。
- `pub` 可修饰 `impl`（影响可见性，与旧 `pub` 语义对齐）。
- **多协议一致性**：`impl T: A, B { .. }` 支持（PC-9）——desugar 按协议成员名裁决拆分为多个 impl
  块（与声明点一致性 `struct C: A, B` 同一规则，见 §7.9）。
- **旧语序 `impl P for T` 已移除**（PC-12，见 §7.12）：语法只接受 `impl T: P` / `impl T`。
- **`extension` / `protocol` 关键字已移除**：语法只接受 `protocol`（协议声明）与 `impl T: P` / `impl T`
  （一致性 / 固有实现）。二者已不再是保留字（PC-11，见 §7.11）。

### 3.5 协议继承：`protocol A: B`

> **现状**：`grammar.md` §2.6 已写 `ProtocolDecl ::= ... ':' ProtocolBound? ...`，但 `parse_protocol` **从未实现** `:` 处理，全仓无 `protocol A: B` 用例——即**规划未落地**。本方案顺带以 Swift 同形语法落地。

```rlyeh
protocol Named {
    fn name(&self) -> String;
}

protocol Loud: Named {          // Loud 继承（要求）Named：实现 Loud 必须同时满足 Named
    fn shout(&self) -> String;
}

struct Dog: Loud {
    fn name(&self) -> String { String::from("dog") }
    fn shout(&self) -> String { String::from("DOG!") }
}
```

语义：`protocol A: B` 表示 **A 的见证者必须同时是 B 的见证者**（`A` 蕴含 `B`）；对 `dyn A` 可安全上转 `dyn B`。约束检查复用 `type_implements_protocol` 的传递闭包。

**已落地（PC-4 / PC-10，2026-09-18）**：

- 一致性义务校验（实现 `A` 必须实现 `B`，否则 `TC029`）+ `type_implements_protocol` 传递闭包（PC-4）；
- **vtable 线性化**：`dyn T` 的方法槽按「superprotocol 方法在前、本协议方法在后（同名去重）」排列，
  因此 **`dyn A` 的 vtable 前缀与 `dyn B` 相同**；
- **`dyn A → dyn B` 静态上转**（值层与 `&dyn A → &dyn B` 引用层同构）——直接复用同一胖指针，
  **仅编译期改类型、零运行时开销**；
- **`dyn 子协议` 接收者可直接调用父协议方法**（其槽位位于 vtable 前部）。

见 §7.10。

### 3.6 关联类型与关联常量

沿用既有成员语法（`type Item;` / impl 内 `type Item = Concrete;`），仅关键字不变：

```rlyeh
protocol Iterator {
    type Item;                                        // 关联类型（细化）
    fn next(&mut self) -> Option<Self::Item>;
}

struct Iter: Iterator {
    type Item = i64;
    fn next(&mut self) -> Option<i64> { Option::None }
}
```

> **决议（2026-09-18）**：**沿用 `type Item;`**，不引入 Swift 的 `associatedtype`。理由：① 与既有实现、std、测试零改动；② Rlyeh 的 `type` 已是类型别名关键字，复用自然；③ 减少新增关键字。

### 3.7 泛型约束（bound）：不变

`T: P` 与 `where` 原样保留（`:` 语义天然一致）：

```rlyeh
fn loud<T>(x: T) -> i64 where T: Speak { x.speak() }
fn both<T: Speak + Named>(x: T) -> i64 { x.speak() + x.name_id() }
```

### 3.8 `dyn` 存在类型与协议组合：保留 `dyn`，组合用 `+`

```rlyeh
let shapes: Vec<dyn Draw> = ...;         // protocol object → dyn 协议
let x: dyn Draw + Named = ...;           // 多协议组合
```

- 关键字 `dyn` **保留**（**决议**：不引入 `any` 别名）。
- 组合运算符用 **`+`**（既有 bound 写法），**不用** Swift 的 `&`——因 Rlyeh 中 `&` 是**引用类型构造符**（`&T` / `&mut T`），`A & B` 会与之产生视觉/语法歧义。

### 3.9 `Self`：限制不变

`Self` 仍**仅允许出现在返回位置**（`fn from(v: T) -> Self`）；参数位置禁用；含 `Self` 签名的方法不可经 `dyn` 调用——与既有约束一致，本 RFC 不改。

### 3.10 与既有关键字/内建协议的关系

- 内建协议 `Drop` / `Any` 一并改称"内建 protocol"（语义不变）。
- std 协议 `From` / `Into` / `Iterator` / `Future` / `Serialize` / `Deserialize` / `Display` / `Error` 全部随之改写（§7）。
- `for` 关键字**仅保留**于 `for x in ...` 循环，不再出现于"实现"语境。

---

## 4. Before / After 全量对照

| # | 现状（protocol / impl） | 方案（protocol / impl） |
|---|----------------------|------------------------------|
| 1 | `protocol Area { fn area(&self) -> f64; }` | `protocol Area { fn area(&self) -> f64; }` |
| 2 | `impl Area for Shape { fn area(&self) -> f64 { .. } }` | `struct Shape: Area { .. }` 或 `impl Shape: Area { .. }` |
| 3 | `impl<T> Wrapper<T> { fn new(v: T) -> Wrapper<T> { .. } }` | `impl<T> Wrapper<T> { .. }`（或 `impl Wrapper<T> { .. }`） |
| 4 | `impl<T> Wrap for Pair<T> where T: Speak { .. }` | `impl<T> Pair<T>: Wrap where T: Speak { .. }` |
| 5 | `impl Drop for Resource { fn drop(&mut self) { .. } }` | `impl Resource: Drop { fn drop(&mut self) { .. } }`（内建协议） |
| 6 | `let x: dyn Draw = ..;` | `let x: dyn Draw = ..;`（不变） |
| 7 | `fn f<T: Speak>(x: T)` | `fn f<T: Speak>(x: T)`（不变） |
| 8 | （superprotocol 规划未实现） | `protocol Loud: Named { .. }`（**新增**） |

---

## 5. 语法变更（EBNF 草案）

```ebnf
(* ── 协议声明（取代 ProtocolDecl） ── *)
ProtocolDecl  ::= Attr* 'pub'? 'protocol' Ident RegionParam? GenParams?
                  (':' ProtocolList)? WhereClause?
                  '{' ProtocolItem* '}'
ProtocolItem  ::= FnDecl
                | 'type' Ident ('=' Type)? ';'          (* 关联类型：声明 / 默认 *)
                | ConstDecl                             (* 关联常量（沿用既有） *)
ProtocolList  ::= ProtocolRef (',' ProtocolRef)*
ProtocolRef   ::= Path GenArgs?

(* ── 类型声明：增加一致性列表 + 允许内联成员 ── *)
StructDecl    ::= Attr* 'pub'? 'struct' Ident RegionParam? GenParams?
                  (':' ProtocolList)? WhereClause?
                  '{' StructMember* '}'
StructMember  ::= FieldDecl | FnDecl | TypeAlias
EnumDecl      ::= Attr* 'pub'? 'enum' Ident RegionParam? GenParams?
                  (':' ProtocolList)? WhereClause?
                  '{' EnumBody '}'

(* ── 扩展块（沿用 ImplBlock，Swift 式语序：类型在前、协议在后） ── *)
ImplDecl      ::= Attr* 'pub'? 'impl' GenParams? Type
                  (':' ProtocolList)? WhereClause?
                  '{' ImplItem* '}'
ImplItem      ::= FnDecl | 'type' Ident '=' Type ';' | ConstDecl
```

要点：

- `Attr*` 前缀统一（`#[derive]` / `#[repr(C)]` / `#[memory(gc)]` 等），与既有解析一致。
- `RegionParam?` 为 B-4 已落地的 region 参数后缀（`struct Foo 'a { }`），与本方案**正交**、可组合：`struct Excerpt 'a: Draw { part: &'a i64 }`。
- `ProtocolList` 中每个 `ProtocolRef` 可带泛型实参：`struct P<T>: Wrap<T> { }`。
- 语法仅接受 `protocol` 声明与 `impl [<G>] Type (: ProtocolList)?` 一致性 / 固有实现；`protocol` / `extension`（PC-11）与旧语序 `impl P for T`（PC-12）均已移除，**AST 不新增顶层节点类型**（仅扩充既有 `AstProtocolDecl` / `AstImplBlock` 字段 + `AstStructDecl` 增加一致性/成员字段）。

---

## 6. 语义映射（零成本 desugar）

本方案是**纯语法重排**，`desugar` 阶段把新写法归一到既有的 `AstProtocolDecl` / `AstImplBlock` 结构，**typecheck / codegen 零改动**：

| 新写法 | desugar 归一结果 |
|--------|------------------|
| `protocol P { .. }` | `AstProtocolDecl { .. }`（原样，仅关键字替换） |
| `protocol A: B { .. }` | `AstProtocolDecl` 增加 `superprotocols: Vec<..>`；typecheck 校验 `A` 见证者含 `B` |
| `struct C: P { fields; members }` | `AstStructDecl { fields, conformances: [P] }` + 内联成员按 §3.3 裁决拆为 `AstImplBlock{ protocol_name: Some(P), type_name: C }` 与 `AstImplBlock{ protocol_name: None, type_name: C }`（固有） |
| `enum E: P { .. }` | 同上（enum 版本） |
| `impl C: P { .. }` | `AstImplBlock { protocol_name: Some(P), type_name: C, .. }` |
| `impl C { .. }` | `AstImplBlock { protocol_name: None, type_name: C, .. }` |
| `impl<T> C<T>: P where ..` | `AstImplBlock { generics, protocol_name: Some(P), type_name: C, .. }`（where 已由既有 `parse_where_clause` 支持） |
| `impl C: A, B { .. }`（多协议） | `AstImplBlock { protocol_name: Some(A), extra_protocols: [(B, ..)], .. }` + desugar 按协议成员名裁决拆分为 `impl A for C` / `impl B for C`（PC-9，见 §7.9） |
| `impl<T> C<T> { .. }`（固有泛型） | `AstImplBlock { generics, protocol_name: None, type_name: C, .. }` |

因此：

- **单态化缓存、接口哈希（`INTERFACE_VERSION`）、vtable 布局、`dyn` 上转、`Self` 替换、默认方法回退** 全部不变。
- 需要新增的**唯一语义能力**是 §3.5 协议继承（`superprotocols` 的传递约束 + `dyn` 上转），其余为新语法糖。

---

## 7. 兼容性与迁移策略

> 迁移面：`crates/rlyeh-std` **17 个 `.rl`**、`tests/` **54 个 `.rl`**、`docs/`（guide/manual/std-lib/semantics）、`examples/`、LSP/fmt/doc 工具、语言技能文档。全部为**机械改写**，可用工具自动完成。

### 7.1 分阶段落地

1. **阶段 P0（已完成）**：`protocol` 为协议声明关键字；扩展块沿用 `impl`（`impl T: P` / `impl T`）。`protocol` / `extension` 关键字已移除（PC-11，见 §7.11），旧语序 `impl P for T` 亦已移除（PC-12，见 §7.12）。除与关键字撞名的标识符需改名外（见 §7.5），迁移后的代码照常编译。
2. **阶段 P1（已完成）**：`rlyeh fmt` 默认即输出新语法（`protocol` / `impl Type: Protocol`）；std / tests / examples / 文档代码块的旧语法已全部迁移（PC-7 / PC-11 / PC-12）。
3. **阶段 P2（弃用告警，已撤销）**：曾对旧写法发 `warning[W002]`；随着 `protocol` / `extension`（PC-11）与旧语序 `impl P for T`（PC-12）从语法中移除，该项已无对象，`--deny-deprecated` 与 `--legacy` 选项一并移除。
4. **阶段 P3（移除，已完成）**：`protocol` / `extension` 关键字（PC-11）与 `impl P for T` 的 `for` 语序（PC-12）均已移除；语法收敛为 `protocol` + `impl T: P` / `impl T`。

### 7.2 迁移映射（工具依据）

```
protocol N { .. }                     →  protocol N { .. }
protocol N: S { .. }                  →  protocol N: S { .. }          （S 此前非法，新能力）
impl P for T { .. }                →  impl T: P { .. }
impl<P> P<A> for T<B> { .. }       →  impl<P> T<B>: P<A> { .. }
impl T { .. }                      →  impl T { .. }                 （不变）
impl<P> T<P> { .. }                →  impl<P> T<P> { .. }            （不变）
impl P for T where C { .. }        →  impl T: P where C { .. }
impl P for T { type A = X; }      →  impl T: P { type A = X; }     （不变）
extension T: P { .. }              →  impl T: P { .. }              （弃用关键字）
extension<T> T<B>: P<A> { .. }     →  impl<T> T<B>: P<A> { .. }     （弃用关键字）
extension T { .. }                 →  impl T { .. }                 （弃用关键字）
extension<T> T<P> { .. }           →  impl<T> T<P> { .. }           （弃用关键字）
```

> **决议（2026-09-18 二次复议）**：迁移工具**默认**将 `impl ... for ...`（及弃用关键字 `extension T: P`）改写为 **`impl T: P`**（而非把成员搬进 `struct T: P`）——不触碰类型声明本体，改写风险最低。`struct T: P` 的内联风格（§3.3）作为**可选手工优化**；如后续确有需要，可另加 `--inline` 选项（默认关闭）。

### 7.3 不可自动改写 / 需人工确认的情形

- `impl` 块中**同时**含关联类型与固有方法（需拆分到 `impl T: P` 与 `impl T`）——工具给出 TODO 注释。
- 同一类型对同一泛型协议的**多 impl**（`impl Wrap<i64> for W` / `impl Wrap<bool> for W`）——工具需保留为两个 `impl W: Wrap<i64>` / `impl W: Wrap<bool>`（注意 `ProtocolRef` 泛型实参随行）。
- `#[derive(..)]` 与手写一致性并存——保持不动（§7.4）。

### 7.4 `#[derive(...)]` 的处置

`#[derive(Clone, Serialize, ...)]` **保持不变**：它生成的是**一致性代码**（等价于自动写入 `impl T: Clone`），与手写 `impl T: P` 正交。可选增强（列开放问题）：允许 `struct C: Clone { }` 直接声明派生型协议，由 `derive` 补齐方法体——本 RFC 不强制。

### 7.5 关键字撞名迁移（2026-09-18）

`protocol` 作为**硬保留字**后，与既有标识符存在 2 处冲突，已就地改名：

| 位置 | 原名 | 新名 | 说明 |
|------|------|------|------|
| `rlyeh-std/rlyeh/core.rl` | `extern fn socket(.., protocol: i64, ..)` 形参 | `proto` | extern 形参名（与 ABI 无关） |
| `examples/projects/chatd/protocol.rl` | 模块名 `protocol` | 模块 `chat_protocol`（文件同名改名） | 同步 `module` / `import` / `README.md` |

> **`extension` 的处置**：早期切片曾把 `extension` 作为新关键字引入，导致 `Path::extension()` 被迫改名 `path_extension`。**2026-09-18 二次复议后 `extension` 转为「过渡期弃用关键字」**（扩展块语法改用 `impl T: P` / `impl T`，见 §7.7）；`Path` 方法名**保留 `path_extension`**（不再回退，避免对已发布 API 造成二次改名扰动）。
>
> **结论**：`protocol` 是唯一**新增**的硬保留字，标识符撞名不可回避。`protocol` / `extension`（PC-11）与旧语序 `impl P for T`（PC-12）均已从语法中移除；迁移可由 `rlyeh fmt` 完成，关键字撞名可用「保留字用作标识符」诊断定位。

### 7.6 PC-0 实现纪要（2026-09-18）

> **注意（2026-09-18 二次复议）**：本节为 PC-0 落地时的**历史纪要**，其中 `extension` 关键字**已转为弃用**、扩展块语法改为 `impl T: P` / `impl T`（见 §7.7）。

首个切片 **PC-0**（关键字 + parser 双写接受）已落地**基础部分**：

- **词法**：`crates/rlyeh-lexer/src/{token.rs,lib.rs}` 新增 `Token::Protocol` / `Token::Extension` 及关键字映射；`tests.rs` 新增 `test_protocol_extension_keywords`。
- **语法**：`crates/rlyeh-parser/src/parser.rs` 的顶层项分派 / `pub` 分派 / `is_item_start` 纳入两关键字；`item.rs` 的 `parse_protocol` 接受 `protocol` **或** `protocol`（归一为 `AstProtocolDecl`）；新增 `parse_extension` 支持
  `extension [<Generics>] Type (: Protocol)? where? { ... }`（归一为 `AstImplBlock`，**零新增 AST 节点**）。
- **已支持**：`protocol P { .. }`；`extension T { .. }`（固有）；`extension T: P { .. }`（单协议一致性）；`extension<G> T<G>: P<G> { .. }`（泛型 + 泛型协议实参）；`where` 子句；关联类型 `type Item = X;`。
- **已知限制（PC-0 层面）**：① 声明点一致性 `struct C: P` 由 **PC-1** 落地；② 多协议一致性（`extension T: A, B` / `struct C: A, B`）规划中（PC-3）；③ 协议继承 `protocol A: B`（PC-4）；④ `rlyeh fmt` 打印/`--migrate` 与弃用告警（PC-5/PC-6）。
- **兼容性**：`protocol` / `impl` 旧写法**完全兼容**（零回归）；3 处关键字撞名已改名（§7.5）。
- **验证**：`rlyeh-lexer` / `rlyeh-parser` 单测（`test_protocol_extension_keywords`、`test_protocol_and_extension_keywords`、`test_extension_generic_conformance`）全过；新增 `tests/run-pass/protocol_basic.rl`（protocol + extension 一致性 / 固有方法 / 默认方法回退，输出 `9`/`100`/`109`）；`cargo build --workspace` 通过；全量 `.rl` 套件（77.2s）+ `cargo test --workspace` 全绿（仅 2 个 wasm 用例因本机 wasi-sysroot 缺 `crt1.o` / `libc.a` 环境性失败，与本次改动无关）。

### 7.7 扩展块语序复议（2026-09-18 二次评审）

**裁决**：扩展块（追溯式一致性 + 固有方法）**不再使用 `extension` 关键字**，改为**沿用 `impl` 的 Swift 式语序**（类型在前、协议在后）：

| 首次裁决（已落地实现） | 二次复议（本裁决） |
|------------------------|--------------------|
| `extension T: P { .. }` | `impl T: P { .. }` |
| `extension T { .. }` | `impl T { .. }` |
| `extension<G> T<G>: P<G> { .. }` | `impl<G> T<G>: P<G> { .. }` |
| 关键字 `extension` | **弃用**（过渡期继续接受并归一到 `AstImplBlock`，后续 breaking 版本移除） |
| 旧 Rust 语序 `impl P for T { .. }` | 过渡期兼容 + 弃用告警 → `impl T: P` |

**理由**：

1. `impl` 本就是 Rlyeh 既有保留字，**复用它零新增关键字**；`extension` 属额外引入的认知负担。
2. Swift 式语序 `impl T: P` 保留"类型在前"的声明点一致性心智（与 `struct T: P` 同构），同时弃掉 `for` 这个与循环重载的词。
3. 减少一次关键字更名引发的标识符撞名（§7.5）。

**不变的部分**：

- `protocol` → `protocol` 的主张**不变**（§3.1）。
- 声明点一致性 `struct C: P` / 内联成员（§3.2 / §3.3）**不变**。
- `protocol A: B` 协议继承（§3.5）**不变**。

**实现落点（已落地，2026-09-18）**：

- **词法**：`Token::Extension` 保留（驱动弃用告警），不再是主关键字；`Token::Impl` 为新语序主关键字。
- **语法**：`crates/rlyeh-parser/src/item.rs` 的 `parse_impl` 支持新语序
  `impl [<G>] Type [<...>] (: ProtocolList)? [where] { .. }`，并以 lookahead（`for` / `<...>for`）
  兼容旧语序 `impl Protocol for Type`；抽出 `skip_type_generic_args` 复用泛型实参消费；
  `parse_extension` 保留为弃用兼容入口（单协议，多协议仍显式报错）。
- **诊断**：`rlyeh-typecheck` 的 `deprecated_keyword_warnings` 覆盖 `protocol` / `extension` 关键字
  与旧语序 `impl ... for ...`（token 级前瞻 `impl_uses_legacy_for_clause`）；新语序 `impl T: P`
  与固有 `impl T` **不**告警；W002 文案更新为「`extension` 已弃用，请改用 `impl Type: Protocol`」
  /「`impl Type for Protocol` 旧语序已弃用」。
- **格式化**：`tools/rlyeh-fmt` 默认输出 `impl Type: Protocol` / `impl Type`；`--legacy` 输出旧写法。
- **迁移**：`rlyeh-std` 18 个 `.rl` + `tests/run-pass/protocol_basic.rl` 的 `extension ...`
  已改写为 `impl ...`（仅行首关键字；`Path::path_extension` 方法名不动）。
- **文档**：`docs/grammar.md` §2.6 产生式改为 `ImplDecl`（新语序 + 旧语序 / `extension` 两条
  过渡兼容分支），标题改为「Protocol 与实现」。
- **验证**：`rlyeh-parser` 新增 `test_impl_new_syntax_conformance` /
  `test_impl_new_syntax_generic` / `test_extension_still_parses_as_impl`；`rlyeh-typecheck` 新增
  `pc8_impl_new_syntax_no_deprecation` / `pc8_extension_and_legacy_impl_for_warn`；`rlyeh-fmt`
  迁移测试更新；protocol 系列 run-pass 输出不变（`9/100/109`、`9/100/25/16`、`9/7/12`、`3/30`）；
  `cargo build --workspace` 通过；全量 `.rl` 套件（80.3s）+ `cargo test --workspace` 全绿
  （仅 2 个 wasm 环境性失败）；`hir-user` 基线重生成（std 源码长度变化致 span 偏移）、
  5 个快照维度全 0 差异。

### 7.8 PC-7 文档与技能同步（2026-09-18）

**面向用户的活文档**已全量改写为 `protocol` / `impl Type: Protocol`：

- **教程**：`docs/guide/05-aggregates-generics.md`（§5.3 全节 + 练习）、`docs/guide/03-basic-syntax.md`
  （`dyn` 协议对象段 + 迭代器段）。
- **手册**：`docs/manual/08-generics.md`（§8.2 / §8.3）、`01-overview.md`、`02-lexical.md`
  （关键字表补 `protocol` / `extension`）、`03-types.md`、`04-expressions-operators.md`、
  `06-functions-closures.md`、`14-limits.md`、`std/json.md`。
- **规范**：`docs/semantics.md`、`docs/module-system.md`、`docs/memory-model.md`、`docs/grammar.md`（§2.6）。
- **标准库**：`docs/std-lib.md`（`protocol` → `protocol` 全量；`impl X for Y` → `impl Y: X`）。
- **门面**：`README.md`、`CODEBUDDY.md`（示例、§3.5.4 / §3.5.5、特性速览状态表）。
- **技能**：`skills/rlyeh-language/references/{language,pitfalls,semantics}.md`。
- **示例说明**：`examples/std-demos/**/README.md`、`examples/by-chapter/README.md`、
  `examples/projects/benchmarks/**`。
- **设计文档**：`docs/design/*.md`、`docs/design/prompts/*.md`。

**有意未改写**：

- **历史过程记录**：`docs/tasks/**`、`docs/stages/**`、`docs/development-plan*.md`、`CHANGELOG.md`
  ——记录当时状态，不追溯改写。
- **实现层错误消息原文**：`CODEBUDDY.md` 中 `` `type X does not implement protocol B` ``（与 typecheck
  实际消息一致，待后续统一诊断措辞时一并调整）。
- **关键字兼容产生式**：`grammar.md` 的 `'protocol' | 'protocol'` 与 `ImplDecl` 两条过渡兼容分支。
- **Rust 对比语境**：基准报告中「Rust protocol 对象」等对 Rust 语言的描述。

> 本轮为**纯文档改动**（无 `.rs` / `.rl` 变更），故不触发编译与测试回归。

### 7.9 PC-9 多协议 impl 实现纪要（2026-09-18）

`impl T: A, B { .. }`（多协议一致性）已落地，`extension` 弃用端同步对齐：

- **AST**：`AstImplBlock` 新增 `extra_protocols: Vec<(String, Vec<AstType>)>`
  （`protocol_name` 为列表首个协议，其余入此）。
- **语法**：`parse_impl` / `parse_extension` 的一致性列表改用 `parse_conformance_list`
  （`Ident GenArgs? (',' Ident GenArgs?)*`），支持 `impl T: A, B`、`impl<T> Pair<T>: Wrap<T>, Show`、
  `impl T: From<i64>, Into<String>` 等；此前「多协议请拆分」的显式报错已移除。
- **desugar**：新增 `lower_impl_conformance`（在 `lower_items` 中处理 `AstItem::ImplBlock`）：
  - 第 0 个协议保留在原块（位置不变），其余协议追加为独立 impl 块；
  - 成员（方法 + 关联类型）按「**首个接受它的协议**」归属（与声明点一致性 `struct C: A, B`
    同一规则，复用 PC-3 的协议成员预扫描 `proto_members`）；
  - 无处归属的成员并入首个协议块——保证 `impl T: A, B { .. }` 与「拆成多个 impl 分别书写」
    在成员全属首协议时等价；
  - 内置协议 / 跨编译单元协议视为「接受全部成员」（沿用 PC-3 语义）。
- **已知限制**：① 成员归属按**名字**匹配，不做签名比对；② 同名成员同属多协议时归入**首个**；
  ③ 无处归属的成员并入首个协议块（`impl T: ..` 本身即一致性块，不产生独立固有 impl）。
- **验证**：`rlyeh-parser` 新增 `test_impl_multi_conformance_list` /
  `test_impl_multi_conformance_generic` / `test_extension_multi_conformance_list`；新增
  `tests/run-pass/protocol_impl_multi.rl`（`impl Sq: Area, Named`：`area`→Area、`name_id`→Named、
  `perimeter` 并入首协议块，输出 `9` / `7` / `12`）；`cargo build --workspace` 通过；
  全量 `.rl` 套件（78.2s）+ `cargo test --workspace` 全绿（仅 2 个 wasm 环境性失败）；
  `ast-user` 基线重生成（`AstImplBlock` 新增 `extra_protocols` 字段）、5 个快照维度全 0 差异。

### 7.10 PC-10 `dyn` 上转实现纪要（2026-09-18）

**`dyn 子协议 → dyn 父协议` 静态上转**已落地（纯编译期类型转换，**零运行时开销**）：

- **vtable 线性化**（`check_expr::linearize_protocol_methods`）：`dyn T` 的方法槽按
  **superprotocol 方法在前、本协议方法在后**排列，同名方法去重（保留基类版本），多级继承递归处理。
  因此 **`dyn A`（`protocol A: B`）的 vtable 前缀与 `dyn B` 逐槽一致**。
- **vtable 填充**（`coerce_to_dyn`）：按线性化顺序填充方法槽；方法声明于父协议时，
  改为从其**父协议 impl**（`impl B for T`）取实现并实例化函数指针。
- **虚调用槽索引**（`check_expr::method::builtin`）：按同一线性化顺序定位 `3 + idx`，与填充顺序严格一致。
  **因此 `dyn 子协议` 接收者可直接调用父协议方法**（槽位位于 vtable 前部）。
- **上转**（`check_stmt` 的 `dyn_superprotocol_upshift` 分支）：目标为 `dyn B`、源为 `dyn A`
  （或 `&dyn A → &dyn B`，引用层同构）且 `A` 的父协议传递闭包含 `B` 时，**复用同一胖指针**、
  仅编译期改类型。
- **兼容性**：无 superprotocol 的协议线性化结果 = 其自身方法声明序，**与既有 vtable 布局完全一致**
  → 全部既有 `dyn` 用例零回归（`ir` / `hir-user` / `run` 等 5 个快照维度全 0 差异，无需重生成）。
- **已知限制**：① 仅支持**上转**（子 → 父）；下转（父 → 子）仍须经 `Any::downcast_ref` 运行期检查；
  ② `&dyn` 作函数参数仍受 H4 胖指针限制；③ 泛型协议的对象化仍不支持（`coerce_to_dyn` 既有约束）。
- **验证**：新增 `tests/run-pass/protocol_dyn_upshift.rl`（`protocol Loud: Named` +
  `struct Dog: Loud, Named`）：`l.shout()` → `22`、`l.name_id()`（父协议方法经 vtable）→ `11`、
  `let n: dyn Named = l`（上转）→ `n.name_id()` → `11`；`cargo build --workspace` 通过、无 lint；
  全量 `.rl` 套件（81.1s）+ `cargo test --workspace` 全绿（仅 2 个 wasm 环境性失败）；
  5 个快照维度全 0 差异。

### 7.11 PC-11 移除 `protocol` / `extension` 关键字（2026-09-18）

**`protocol` 与 `extension` 已从语法中彻底删除**——二者不再是保留字（可作普通标识符），
声明形式不再可解析：

- **词法**：`rlyeh-lexer` 删除 `Token::Protocol` / `Token::Extension` 变体与关键字映射。
- **语法**：`parse_protocol` 仅接受 `protocol`；`parse_extension` 函数整体删除；`parser.rs` 的
  顶层 / `pub` 分派与 `is_item_start` 相应收窄。
- **诊断**：弃用告警仅保留**旧 impl 语序** `impl P for T` → `impl T: P`（`protocol` / `extension`
  已非关键字，无对应告警）。
- **格式化**：`rlyeh fmt` 始终输出 `protocol`；`--legacy` 现仅影响 impl 语序
  （`impl T: P` → `impl P for T`）。
- **工具**：`rlyeh-doc` 的文档分类标签 `Protocol` → `Protocol`。
- **迁移**：`tests/**`（compile-pass / compile-fail / run-pass 及 `benches`、驱动集成测试的内联源码）
  的 `protocol X` → `protocol X`；std / examples 此前已迁移（PC-7 / PC-8）。
  **注意**：此前迁移正则要求关键字后跟空白，遗漏了 `extension<T>`（无空格）形式，本轮一并补齐。
- **旧语序**：本轮仍保留 `impl P for T`（过渡期 + 弃用告警）；其后已由 **PC-12** 一并移除（见 §7.12）。
- **验证**：`rlyeh-lexer` 单测更新为 `test_protocol_keyword`；`rlyeh-parser` 新增
  `test_protocol_extension_are_plain_identifiers`（二者可作变量名）与 `test_protocol_and_impl_keywords`；
  `rlyeh-typecheck` 改为 `deprecated_impl_for_warn`（仅旧语序触发 1 次告警）；`rlyeh-fmt` 迁移测试
  更新为仅覆盖 impl 语序；`cargo build --workspace` 通过；`cargo test --workspace` 全绿
  （仅 2 个 wasm 环境性失败；`executor_test` 的 fd 时序用例偶发 flaky，单独运行通过）；
  `ast-user` / `hir-user` 基线重生成（用例源码由 `protocol` 改为 `protocol` 致 span 偏移）、
  5 个快照维度全 0 差异。

### 7.12 PC-12 移除旧 impl 语序 `impl P for T`（2026-09-18）

**Rust 式旧语序 `impl Protocol for Type` 已从语法中彻底移除**——`impl` 块**唯一语序**为
`impl [<G>] Type (: ProtocolList)? WhereClause? { .. }`：

- **语法**：`parse_impl` 只解析新语序（`first` 即被实现类型）；删除 `for` 分支与
  `looks_like_generic_protocol_impl` lookahead 辅助。
- **诊断**：`WarningKind::DeprecatedKeyword`（`W002`）与词法扫描 `deprecated_keyword_warnings`
  整体删除——旧写法已不存在、无告警对象。
- **CLI**：driver 的 `--deny-deprecated` 与（driver `fmt` 子命令 / `rlyeh-fmt` 的）`--legacy`
  选项一并移除。
- **格式化**：`FmtOptions.legacy` / `Printer.legacy` 删除，`rlyeh fmt` 只输出新语序。
- **迁移**：`tests/**`（run-pass / compile-pass / compile-fail）、`examples/**`、`crates/**` 的
  `.rl` 共 **88 处** `impl P for T`（含 `impl<T> P<T> for T<T>` 等泛型形式）改写为 `impl T: P`；
  驱动集成测试内联源码（`executor_test` / `join_all_fut_test` / `time_test` / `agg_test`）同步。
- **验证**：`rlyeh-parser` 单测更新（新语序解析）；`rlyeh-typecheck` 删除弃用告警测试；`rlyeh-fmt`
  新增 `new_syntax_stable`（新语法格式化幂等）；`cargo build --workspace` 通过；`cargo test --workspace`
  全绿（仅 2 个 wasm 环境性失败）；`ast-user` / `hir-user` 基线重生成（用例源码语序变更致 span 偏移）、
  5 个快照维度全 0 差异。

---

## 8. 影响面与落点清单

| 层 | 落点 | 改动 |
|----|------|------|
| 词法 | `crates/rlyeh-lexer/src/{token.rs,lib.rs,tests.rs}` | 新增 `Protocol` 保留字；`Impl` 保留（改语序）；`Protocol` / `Extension` 过渡期弃用 |
| 语法 | `crates/rlyeh-parser/src/item.rs` | `parse_protocol` 接受 `protocol`（含 `:` 继承）；`parse_impl` 支持 `impl T: P` 语序（兼容 `for` 旧语序）；`parse_extension` 转为弃用兼容；`parse_struct`/`parse_enum` 增加一致性列表与内联成员 |
| AST | `crates/rlyeh-ast/src/lib.rs` | `AstProtocolDecl` 加 `superprotocols`；`AstStructDecl`/`AstEnumDecl` 加 `conformances` + `members`；旧字段兼容或统一 |
| desugar | `crates/rlyeh-desugar/src/**` | 归一化（§6）：内联成员拆分、`impl T: P` → `AstImplBlock` |
| typecheck | `crates/rlyeh-typecheck/src/**` | 仅新增**协议继承传递约束**；其余复用 |
| codegen | `crates/rlyeh-codegen/**` | **零改动**（语法糖） |
| 工具 | `tools/rlyeh-fmt`（打印 + `--migrate`）、`tools/rlyeh-doc`、`tools/rlyeh-check`、`crates/rlyeh-lsp` | 关键字/打印/迁移支持 |
| 标准库 | `crates/rlyeh-std/**`（17 文件） | 机械改写 |
| 测试 | `tests/**`（54 文件，含 compile-fail 的 `// expect:` 片段） | 机械改写 + 新增正/反向用例 |
| 示例 | `examples/**` | 机械改写 |
| 文档 | `grammar.md`、`semantics.md`、`std-lib.md`、`guide/05`、`manual/08`、`design/03_类型系统.md`、`README`/`CODEBUDDY.md` | 规范与教程改写；本 RFC 评审通过后并入权威规范 |
| 技能 | `skills/rlyeh-language/**` | 同步语法描述 |

---

## 9. 任务拆解

| ID | 任务 | 优先级 | 风险 | 依赖 |
|----|------|--------|------|------|
| PC-0 | 关键字 `protocol` 词法 + parser 双写接受（**基础已落地**，2026-09-18；旧语法归一） | P0 | 低 | — |
| PC-1 | `struct`/`enum` 声明点一致性列表 `: P, Q` 解析 + AST | P0 | 中 | PC-0 |
| PC-2 | 扩展块解析（追溯一致性 + 固有方法 + 泛型 + `where`）（**已落地**，2026-09-18；语法形态经 §7.7 复议为 `impl T: P`） | P0 | 中 | PC-0 |
| PC-3 | desugar 归一化（内联成员裁决拆分 → 既有 impl 结构） | P0 | 中 | PC-1/PC-2 |
| PC-4 | 协议继承 `protocol A: B`（superprotocols 传递约束 + `dyn` 上转）（**传递约束已落地**；`dyn` 上转由 PC-10 落地，见 §7.10） | P1 | 中 | PC-1 |
| PC-5 | `rlyeh fmt --migrate` 自动改写工具 + std/tests/examples 迁移 | P1 | 中 | PC-0..PC-3 |
| PC-6 | 弃用告警（旧关键字 `warning`，**0.x 内发布**）→ 后续版本移除 | P2 | 低 | PC-5 |
| PC-7 | 文档/技能全量改写并入权威规范（**活文档已落地**，2026-09-18，见 §7.8；历史过程记录不改写） | P1 | 低 | PC-0..PC-4 |
| PC-8 | **扩展块语序复议**：`extension T: P` → `impl T: P`、`extension T` → `impl T`，`extension` 转为弃用（**已落地**，2026-09-18，见 §7.7） | P0 | 中 | PC-0..PC-6 |
| PC-9 | **多协议 impl**：`impl T: A, B`（含 `extension` 端）支持 + desugar 按成员名裁决拆分（**已落地**，2026-09-18，见 §7.9） | P1 | 中 | PC-8 |
| PC-10 | **`dyn` 上转**：vtable 线性化（父协议方法槽在前）+ `dyn A → dyn B` 静态上转（**已落地**，2026-09-18，见 §7.10） | P1 | 中高 | PC-4 |
| PC-11 | **移除 `protocol` / `extension` 关键字**（词法 / 语法 / 诊断 / 格式化 / 工具 / 用例 / 文档）（**已落地**，2026-09-18，见 §7.11） | P1 | 中 | PC-8 / PC-9 |
| PC-12 | **移除旧 impl 语序 `impl P for T`** + 弃用告警（`W002`）/ `--deny-deprecated` / `--legacy` 设施删除（**已落地**，2026-09-18，见 §7.12） | P1 | 中 | PC-11 |

建议首轮切片：**PC-0 → PC-1 → PC-2 → PC-3**（语法可用 + 零破坏），再推进 PC-4/PC-5。

---

## 10. 验收标准

- `protocol` / `impl T: P` 语法**端到端可用**（parse → typecheck → codegen → run）。
- 声明点一致性 `struct C: P { }` / `enum E: P { }` 与 `impl C: P { }` 行为**完全等价**于旧 `impl P for C`。
- 固有方法 `impl C { }` 等价于旧 `impl C`。
- 协议继承 `protocol A: B` 落地：缺失 `B` 成员报错；`dyn A`→`dyn B` 上转可用。
- 过渡期**旧语法 100% 兼容**（全量 740+ `.rl` 用例零回归）；`rlyeh fmt --migrate` 改写后**输出与运行结果不变**。
- 既有能力（默认方法 / 关联类型 / 泛型协议与约束 / `dyn` / `Self` 返回位）**语义零变更**。
- `grammar.md` / `semantics.md` / 教程 / std-lib 全量同步；`CODEBUDDY.md` 特性速览更新。

---

## 11. 开放问题

### 11.1 已决议（2026-09-18 评审）

1. **关联类型语法**：**沿用 `type Item;`**，不引入 `associatedtype`（零 churn、复用 `type` 关键字）。
2. **存在类型关键字**：**保留 `dyn P`**，不引入 `any` 别名。
3. **迁移默认风格**：`rlyeh fmt --migrate` **默认改写为 `impl C: P`**（保守，不触碰类型声明本体）；内联到 `struct C: P` 仅作可选/手工优化。（**二次复议更新**：原决议为 `extension C: P`，语序复议后随关键字一并改为 `impl C: P`，见 §7.7。）
4. **关键字退役节奏**：**在 0.x 即发布弃用告警**（`W0xx`）；移除放到后续 breaking 版本，不推迟到自举之后。
5. **扩展块关键字**（**二次复议**）：**弃用 `extension`，改用 `impl` 的 Swift 式语序**（`impl T: P` / `impl T`），见 §7.7。

### 11.2 待定

6. **协议组合的上下文消歧**：`dyn A + B`（类型位置）与 `T: A + B`（约束位置）均用 `+`；混用处是否需要消歧诊断？
7. **`protocol` 的可见性修饰**：除 `pub` 外是否需要更细的可见性？（当前沿用既有规则。）
8. **迁移工具是否合并同类型的多个 `impl` 块**？（跨文件合并有风险，倾向不合并。）

---

## 12. 附录

### 12.1 附录 A：关键字增删

- **新增保留字**：`protocol`。
- **保留并改语序**：`impl`——唯一语序为 `impl T: P` / `impl T`（旧 `impl P for T` 语序已移除，PC-12）。
- **已移除**：`protocol`（→ `protocol`）、`extension`（→ `impl T: P` / `impl T`）——不再是保留字（PC-11）。
- **保持不变**：`type`、`Self`、`dyn`、`where`、`for`（仅循环）、`pub`。

### 12.2 附录 B：术语对照

| Rlyeh（现状） | Rlyeh（本方案） | Swift | Java/C# | Rust |
|---------------|-----------------|-------|---------|------|
| `protocol P` | `protocol P` | `protocol P` | `interface P` | `protocol P` |
| `impl P for T` | `struct T: P` / `impl T: P` | `struct T: P` / `extension T: P` | `class T implements P` | `impl P for T` |
| `impl T`（固有） | `impl T` | `extension T` | （类体） | `impl T` |
| `protocol A: B`（规划未实现） | `protocol A: B` | `protocol A: B` | `interface A extends B` | `protocol A: B` |
| `type Item;` | `type Item;` | `associatedtype Item` | （泛型） | `type Item;` |
| `dyn P` | `dyn P` | `any P` | `P`（接口引用） | `dyn P` |
