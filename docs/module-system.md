# Rlyeh 模块系统规范

> **状态**：设计稿 v0.1.0（P1–P4 渐进落地）
> **标注**：✅ 已实现 ｜ 🔧 部分实现 ｜ 📋 规划
> **权威文档**：语法 EBNF 见 `docs/grammar.md` §2.2；本文件为模块系统的语义与编译模型权威规范。
> **v0.1。0 变更（2026-08-25）**：关键字 `mod` → `module`、`use` → `import`；目录模块文件名约定 `mod.rl` → `module.rl`（与关键字一致）；可见性仅 `pub` / 私有（不支持 `pub(crate)` / `pub(super)`）；无 `crate` 关键字，路径系统为「模块名::」；模块为扁平名字空间。

---

## 1. 概述与设计目标

模块系统是 Rlyeh 代码组织与复用的第一等公民。MVP 已具备最小可用的 `module` / `import` / 多文件模块能力，本设计在**不破坏现有语法与存量代码**（存量代码随关键字更名同步迁移）的前提下，补齐导入形态、可见性、编译模型与包集成四块能力，使模块系统达到生产级。

**设计目标**（对齐 Rlyeh 哲学——简单、直觉、数学式）：

| 目标 | 说明 |
|------|------|
| 直觉 | `module` / `import` 关键字贴近自然语言；路径「模块名::」明确无歧义 |
| 扁平 | 模块是**扁平名字空间**：无 `crate` / `super` / `self` 层级寻址，路径一律从模块名开始 |
| 渐进 | 每个阶段独立可落地、可验证，存量代码随更名一次迁移 |
| 增量 | 编译模型从文本级展开演进为模块图 + 接口缓存，支持增量编译 |
| 包复用 | 与 dagon 包管理器打通：`Rlyeh.toml` 声明依赖 → 依赖模块图 → 编译 |

**非目标**（规划外）：宏卫生 / 过程宏、`#[derive]` 自定义、动态加载（`dlopen` 式插件）、循环依赖（坚决拒绝）。

---

## 2. 现状盘点（MVP ✅）

| 能力 | 状态 | 说明 |
|------|------|------|
| `module name;` 外部文件 | ✅ | `module foo;` → 加载 `foo.rl` 或 `foo/module.rl` |
| `module name { ... }` 内联 | ✅ | 块内可嵌套任意 ModuleItem |
| 多级外部模块 | ✅ | `foo/module.rl` 内可再 `module bar;` → `foo/bar.rl` |
| `import path;` 单路径导入 | ✅ | 含 `as` 别名（`import math::square as sq;`） |
| 跨模块路径访问 | ✅ | `module_name::CONST` / `module_name::Enum::Variant` / `module_name::Type` |
| 文本级模块展开 | ✅ | `rlyeh-driver/src/module.rs` 递归替换 `module name;` → 内联子模块源码 |
| 循环引用检测 | ✅ | 文本加载期 visited 集合 |
| 可见性控制 | 🔧 | `pub` 有语法，**无模块级可见性检查**（扁平名字空间，全部可达） |
| `import a::{b, c}` 组导入 | ✅ | 2026-09-02 实现（`parse_use` 组解析 + `register_use` 逐成员登记）；2026-09-04 支持**嵌套子组** `import a::{b::{x, y}, c}`（`AstUseMember` 递归结构 + `parse_use_group`/`register_use_group` 递归，仅叶子名入作用域） |
| `import a::*` glob 导入 | ✅ | 2026-09-02 实现（`register_use` 枚举模块直接子项） |
| `pub import` 再导出 | ✅ | 2026-09-02 实现（`is_pub` 登记 `prefix::local → 目标全名`，`resolve_full_name` 传递追踪链）；**2026-09-18 路径解析规则**：模块**内**的 `import` 优先按**相对本模块**解析（`import duration::Duration;` → `time::duration::Duration`，故模块内引用自身子模块无需写全路径），仅当相对目标不存在时才视为**跨模块绝对路径**（如 `module outer` 内 `import inner::secret;` 引用顶层 `inner`）；模块**外**（顶层）导入写完整路径 |
| 外部包依赖编译 | 📋 | dagon 已能 resolve/lock，但 `rlyeh build` 未注入依赖模块路径 |

**现有实现要点**（供设计对齐）：
- 名称解析：`rlyeh-typecheck` 以 `module_prefix` 字符串拼接扁平符号空间（`full_name(prefix, name)` → `module_name::item`）。
- 文件加载：`rlyeh-driver/src/module.rs` 对每个文件做文本变换（含 `span` 合并，错误定位尚可）。
- 增量基础：`incremental/hash.rs` 已有 `ModuleInterface`（函数签名哈希），供后续模块接口缓存复用。
- **目录模块文件名约定**：`foo/module.rl`（与关键字 `module` 一致，对齐 Rust `mod.rs` 惯例，保持不变）。

---

## 3. 语法设计

### 3.1 模块声明

```
ModuleDecl  ::= 'module' Ident ';'                    // 外部文件模块（现有 ✅）
              | 'module' Ident '{' ModuleItem* '}'    // 内联模块（现有 ✅）
              | 'pub' ModuleDecl                      // 📋 公开模块（对外部依赖可见）
```

- `module name;` 的文件解析规则（现有）：`name.rl` 优先，其次 `name/module.rl`（含多级展开）。
- 📋 规划：支持 `#[path = "..."]` 显式指定文件（对齐现有目录化 std 布局，非必需 MVP）。

### 3.2 import 导入

```
ImportDecl  ::= 'import' ImportTree ';'
              | 'pub' 'import' ImportTree ';'     // 📋 再导出（re-export）

ImportTree  ::= Path                    // 单路径（现有 ✅，含 `as` 别名）
              | Path ':' ':' '{' ImportList '}'    // 组导入 📋
              | Path ':' ':' '*'           // glob 导入 📋
ImportList  ::= ImportTree (',' ImportTree)* ','?
```

**示例**：

```rlyeh
// 单路径 + 别名（现有 ✅）
import math::PI;
import math::square as sq;

// 组导入 📋
import math::{PI, square as sq, sqrt};

// glob 导入 📋（导入模块内全部可见符号）
import math::*;

// 再导出 📋（子模块聚合为库对外 API）
pub import inner::{vec::Vec as V, map::HashMap as M};
```

### 3.3 路径系统（扁平名字空间）

```
Path        ::= Ident ('::' Ident)*
```

- **无 `crate` / `super` / `self` 关键字**：路径首段恒为**模块名**，从扁平模块名字空间解析。
- 路径形态：`模块名::item`、`模块名::子模块名::item`、模块内裸名（见 §4.2）。
- 嵌套模块的完整路径自然为 `外层::内层::item`（首段仍是模块名，扁平无歧义）。
- 现有裸名相对语义（模块内符号直接引用）保留。

> **决策（v1.1）**：Rlyeh 模块是**扁平名字空间**——不存在 `crate::` / `super::` / `self::` 相对寻址。所有跨模块引用一律以模块名开头（`模块名::...`），模块名在程序内唯一；解析不依赖"当前模块所在层级"，杜绝歧义与重名链。

### 3.4 可见性

```
Visibility  ::= 'pub'
```

- **默认私有**（📋）：符号仅当前模块及其子模块可达。MVP 现状为全部可达，P2 收紧为默认私有 + `pub` 两级。
- **仅 `pub` / 私有两档**：不支持 `pub(crate)` / `pub(super)`（扁平名字空间下无 crate / super 层级概念）。
- 应用于：`fn` / `struct` / `enum` / `protocol` / `const` / `static` / `module` / `import`（再导出）。

---

## 4. 语义设计

### 4.1 模块名空间与符号表

每个 crate（`rlyeh build` 的入口）由一组模块组成，**模块名全局唯一（扁平）**：

```
模块名空间：math | fs | fs::path（嵌套） | cache
符号表：   math::PI / fs::read_file / fs::path::Path / cache::Entry
```

- **符号命名空间**：类型与函数/常量同一命名空间（与 Rust 一致），`import` 别名冲突报错；模块名与符号名可同名（解析按 §4.4 遮蔽规则）。
- **跨模块可达性** = 模块名空间路径（`fs::path::Path`）＋ `import` 别名注入。
- 符号名编码（`module_name::item` 扁平串）为代码生成契约，不随模块树演进改变（现有 ✅）。

### 4.2 名称解析算法（📋 P2）

对每处名称引用 `R`，按序解析：

1. **本地作用域**：函数参数 / 局部绑定 / `for` 模式（现有 ✅）。
2. **当前模块符号与直接 `import` 别名**（现有 ✅）。
3. **模块名路径**：路径首段查扁平模块名空间，命中则逐段下钻（`模块名::子模块::item`，现有 `module_prefix` 拼接，扩展为名字空间表查找）；未命中 → 步骤 4。
4. **glob 注入**（✅）：`import m::*` 在当前模块注册 m 的全部可见符号；与显式符号冲突时显式优先（glob 不遮蔽显式绑定）。
5. **未找到** → 报 `NameNotFound`（给出候选：拼写相近符号 + 依赖未声明提示）。

> 注意：无 `crate` / `super` / `self` 前缀分支——路径首段就是模块名，扁平解析，无相对层级。

### 4.3 可见性检查（📋 P2）

- 私有符号仅允许：定义所在模块、其子模块、以及（若为嵌套模块）祖先链上的引用。
- `pub` 符号：整个程序 / 依赖图内可见。
- 跨模块访问私有符号 → `PrivateItem` 错误（含"建议加 `pub`"提示）。
- **子模块可访问祖先的私有项**（向下开放、向上封闭）。

### 4.4 歧义与遮蔽

| 场景 | 规则 |
|------|------|
| `import` 别名与本地符号冲突 | 报 `NameConflict`（明确报错，不静默遮蔽） |
| 两个 glob 导入同名 | 两边都注入，使用处歧义报错（提示逐段显式化） |
| 显式符号与 glob 同名 | 显式优先（glob 不遮蔽） |
| 嵌套模块同名符号 | 最近模块优先（depth-first，现有行为保留） |
| 模块名与符号名同名 | 符号优先（路径首段先查符号表再查模块名空间） |

### 4.5 循环引用

- **文本加载期**（现有 ✅）：`module.rs` visited 集合拒绝文件级 `module a;` → `module b;` → `module a;` 无限展开。
- **模块图期**（📋 P3）：模块依赖有向图拓扑排序，环 → `CyclicModule` 错误（列出环路径）。
- 语义层循环（`a` 的函数调用 `b` 的函数且互调）**允许**：函数体内交叉引用经编译期符号表统一解析，不构成编译错误（与 Rust 一致）。

---

## 5. 编译模型

### 5.1 现状：文本级递归展开（✅）

`rlyeh-driver/src/module.rs`：把 `module name;` 替换为 `module name { <文件源码> }` 后递归展开，最终交给单文件流水线。优点：改动小、错误定位经 span 合并基本可用。缺点：每文件重复 parse、无模块级缓存、无增量、无可见性。

### 5.2 目标：模块图编译（📋 P3）

```
1. 模块发现    递归扫描 module 声明 → 模块名空间表 + 文件路径表
2. 依赖排序    模块依赖图拓扑排序（拒绝环）
3. 分片编译    每模块 parse → 接口收集 → 独立缓存（对齐 incremental/hash.rs ModuleInterface）
4. 整体检查    可见性检查 + 跨模块类型统一（protocol impl 跨模块一致性）
5. 代码生成    与现有单文件流水线一致，符号名保持 `模块名::item` 扁平编码（代码生成零改动）
```

- 模块接口 = 公开符号的签名集合（函数签名 / 结构体布局 / 常量值 / protocol 定义）。
- 依赖模块变更 → 仅重新编译依赖方 + 失效接口哈希（增量编译，对齐现有 `--cache-dir`）。

### 5.3 错误报告

- 定位保持文件/行列（现有 span 合并机制沿用）。
- 新增错误类型：`PrivateItem` / `NameNotFound`（带候选）/ `NameConflict` / `CyclicModule`。
- 组导入 / glob 中的单项错误精确定位到 `ImportTree` 项。

---

## 6. 包与依赖集成（📋 P4）

现状：`dagon` 支持 `Rlyeh.toml`（package / dependencies / dev_dependencies）+ `Rlyeh.lock`（PubGrub 求解）+ 本地/HTTP registry，但 `rlyeh build` 仅编译入口文件，**依赖源码未注入编译**。

设计：

```
Rlyeh.toml
  [dependencies]
  foo = "0.1"          # registry 名/版本

dagon resolve → Rlyeh.lock → 下载源码到 registry 缓存（现有 ✅）
                              ↓
rlyeh build 时：
  1. dagon 传入依赖根列表（--dep-root <pkg>=<dir> 多段）
  2. driver 将每个依赖的 src/lib.rl 作为"虚拟模块集合"载入模块名空间
  3. 依赖模块经可见性（pub）暴露，主程序以模块名访问（import foo::bar;）
  4. 依赖接口缓存 → 未变更的依赖零重编译
```

- **依赖命名空间**：`import foo::bar;` 中首段 `foo` 由依赖注入（`--dep-root foo=...` 建立 `foo` → 依赖模块根映射），与扁平模块名空间合并。
- **版本冲突**：PubGrub 已解决（lock），编译期无重复定义。
- **std 不变量**：标准库走 `RLYEH_STD_PATH`（现有机制），不进入依赖图。

---

## 7. 演进路线

| 阶段 | 内容 | 主要改动 | 验收 |
|------|------|----------|------|
| **P0 关键字更名** | `mod` → `module`、`use` → `import` 全链路迁移（✅ 2026-08-25 已随本设计落地） | lexer 关键字表 / driver 展开 / std / examples / tests / docs | 全量测试回归通过 |
| **P1 语法补齐** | 组导入 / glob / `pub import`（✅ 2026-09-02）· `pub module`（⏳ 未实现） | parser（parse_module / parse_import）+ AST + typecheck 路径解析 | 组/glob/pub-import 全部形态可解析、可 typecheck、可生成可运行代码 |
| **P2 可见性** | 默认私有 + `pub` 检查（仅两档） | typecheck 可见性表 + 检查器 | 私有访问报错；`tests/compile-fail` 用例通过 |
| **P3 编译模型** | 模块图编译 + 模块接口缓存 + 增量 | `rlyeh-driver` module.rs 重构 + incremental 扩展 | 模块级增量生效；环形模块报错 |
| **P4 包集成** | dagon 依赖注入编译 + 依赖命名空间 | dagon build + driver `--dep-root` | `rlyeh build` 直接编译含第三方依赖的项目 |

**依赖顺序**：P1 ⊃ P2 ⊃ P3 ⊃ P4（P4 依赖 P3 的模块图，P3 依赖 P1 的路径解析）。

**落地优先级建议**：P1 立即开始（改动局部、收益明显：组导入 + glob + 可见性语法）；P2 与 P3 可并行设计；P4 排最后。

---

## 8. 兼容性

- **存量代码迁移**（P0 已随 v1.1 完成）：`mod` → `module`、`use` → `import` 全局替换；目录模块文件名约定同步为 `module.rl`（std 8 个目录模块文件已重命名），`module name;` 文件解析规则（`name.rl` → `name/module.rl`）沿用。
- 裸名相对路径语义（当前模块符号直接引用）不变。
- 标准库目录化模块（`module.rl` + `core/` + `<name>/module.rl`）与 P3 模块图兼容。
- 类型检查符号命名（`module_name::item` 扁平编码）为代码生成契约，P1–P4 均不改动该契约。

---

## 9. 参考实现对照

| 文件 | 职责 |
|------|------|
| `crates/rlyeh-parser/src/item.rs` | `parse_mod` / `parse_use`（P1 主要改动点；内部函数名保留） |
