# Zeta CodeBuddy Prompt 索引

> 本目录包含 13 个可独立执行的 Prompt 文件，每个对应编译器/工具链的一个核心模块。

---

## 快速导航

| 编号 | 文件 | 模块 | 预估工期 |
|------|------|------|----------|
| P001 | [P001_词法分析器核心.md](./P001_词法分析器核心.md) | `zeta-lexer` | 3-5 天 |
| P002 | [P002_语法分析器核心.md](./P002_语法分析器核心.md) | `zeta-parser` | 5-7 天 |
| P003 | [P003_比较链语义分析.md](./P003_比较链语义分析.md) | `zeta-typecheck` | 3-5 天 |
| P004 | [P004_区域系统实现.md](./P004_区域系统实现.md) | `zeta-regionck` | 5-7 天 |
| P005 | [P005_Transfer语义实现.md](./P005_Transfer语义实现.md) | `zeta-regionck` | 3-5 天 |
| P006 | [P006_Actor运行时.md](./P006_Actor运行时.md) | `zeta-actor-runtime` | 5-7 天 |
| P007 | [P007_增量编译引擎.md](./P007_增量编译引擎.md) | `zeta-driver` | 5-7 天 |
| P008 | [P008_包管理器Zep.md](./P008_包管理器Zep.md) | `zep` | 5-7 天 |
| P009 | [P009_标准库核心模块.md](./P009_标准库核心模块.md) | `zeta-std` | 5-7 天 |
| P010 | [P010_智能区域分配器.md](./P010_智能区域分配器.md) | `zeta-region-alloc` | 5-7 天 |
| P011 | [P011_MIR中间表示实现.md](./P011_MIR中间表示实现.md) | `zeta-mir` | 5-7 天 |
| P012 | [P012_L0借用检查器实现.md](./P012_L0借用检查器实现.md) | `zeta-borrowck` | 3-5 天 |
| P013 | [P013_LLVM后端与代码生成.md](./P013_LLVM后端与代码生成.md) | `zeta-lir`+`zeta-codegen`+`zeta-driver` | 7-10 天 |

**总计**：约 59-98 天（单人），2-3 个月（3-4 人团队）

---

## 依赖关系

```
P001 ──→ P002 ──→ P003 ──→ P004 ──→ P005 ──→ P012 ──→ P011 ──→ P013
                                       │
                              P006 ←───┤
                                       │
                        P007 ──→ P008 ──→ P009
                           │
                           └───→ P010
```

**关键路径**：P001 → P002 → P003 → P004 → P005 → P007 → P008 → P009（P012/P011/P013 为编译器主线支路）

> 注：P006 的精确前置依赖为 P004；P007 的精确前置依赖为 P002-P005；P010 的精确前置依赖为 P004 + P007（P007 已隐含 P004，故图中从 P007 引出）；P012 的前置为 P005（Transfer 语义提供所有权转移路径）；P011 的前置为 P004 + P005 + P012（MIR 显式化区域与 transfer 指令）；P013 的前置为 P011（LIR lowering 依赖 MIR CFG）。

---

## 使用方式

1. **打开**本文件，选择要执行的 Prompt
2. **打开**对应的 `Pxxx_*.md` 文件
3. **全选复制**文件内容
4. **粘贴到 CodeBuddy**
5. CodeBuddy 会按文档要求生成代码
6. 跑通测试后进入下一个 Prompt

---

## 进度追踪

> 在下方表格中记录每个 Prompt 的完成状态

| Prompt | 状态 | 完成日期 | 测试通过率 | 备注 |
|--------|------|----------|------------|------|
| P001 | ✅ 完成 | 2026-08-19 | 35/35 (100%) | 20 单元 + 14 集成 + 1 doc |
| P002 | ✅ 完成 | 2026-08-19 | 62/62 (100%) | 44 单元 + 15 集成 + 3 伪模糊；clippy 零警告；fuzz 发现并修复 const EOF panic |
| P003 | ✅ 完成 | 2026-08-19 | 100% | 比较链 + in 表达式语义 |
| P004 | ✅ 完成 | 2026-08-19 | 100% | bump allocator + 四策略 + LIFO 析构 + regionck 嵌套/归属/transfer |
| P005 | ✅ 完成 | 2026-08-20 | 100% | transfer 嵌套方向/PartialTransfer 语义；遗留 use-after-move 由 P012 补齐 |
| P006 | ✅ 完成 | 2026-08-20 | 15/15 (100%) | Actor 运行时（工作窃取调度 + 邮箱互斥 + ask/reply + Supervisor 恢复 + Router/Timer + 优雅关闭）；补齐 M2.1 |
| P007 | ✅ 完成 | 2026-08-20 | 24/24 (100%) | 增量编译（源码/接口哈希 + 多版本产物缓存 + 损坏恢复 + 依赖图 + 多文件模块缓存）；补齐 M2.3 |
| — | ✅ 完成 | 2026-08-20 | 7/7 (100%) | 模块系统（嵌套 `mod` + `use` 导入别名 + `mod foo;` 多文件加载 + 扁平符号名 + 模块内符号解析）——parser/typecheck/driver/codegen 跨层联动 |
| P008 | ✅ 完成 | 2026-08-20 | 29/29 (100%) | 包管理器 Zep（pubgrub 依赖解析 + 本地/HTTP 注册表 + 打包解包 + 构建驱动）；补齐 M2.2 |
| — | ✅ 完成 | 2026-08-20 | 10/10 (100%) | 聚合对象语言特性（enum + match + impl + trait + 泛型单态化）：HIR `Alloc`/`FieldGet`/`FieldSet` 堆对象原语 + tag 槽表示 + match 展开为 if-else 链 + `self`/`Self` 方法 + trait/impl 收集 + 调用点单态化（`unify`/`substitute`）+ MIR/LIR/codegen 全链路（malloc/GEP/bitcast + f64 槽转换） |
| — | ✅ 完成 | 2026-08-20 | 62/62 (100%) | 聚合对象语言特性扩展：① `impl<T> Foo<T>` 泛型 self 类型解析；② `while`/`for`/`loop`/`region` 语句式（parse_block 语句集合扩展）；③ **struct 字面量构造 `Point { x, y }` + 字段访问/赋值全链路**（AST `StructCtor` + parser lookahead 歧义消除 + typecheck `check_struct_construct`/`check_field_access` + FieldGet 赋值目标）；④ `&self`/`&mut self` 引用接收者方法 + `Type::method()` 静态方法；⑤ match scrutinee 引用自动解引用（`match self`）+ match arm 外层变量可见性；⑥ `loop` 表达式类型 Never 化；⑦ 泛型替换下沉到模式绑定（`check_pattern` 应用 `generic_subst`）+ `Option::None` 的 `_`(Infer) 占位 + `compatible_with` Infer 宽松 |
| — | ✅ 完成 | 2026-08-20 | 63/63 (100%) | 标准库预置接入 + 聚合类型全链路修复：① **标准库搜索路径**（`stdlib.rs` 自动注入 `core.zeta` + `--no-std` + 缓存键覆盖 std 源码）；② **LIR 跨函数类型解析覆盖语义 + 副本类型传播**（未推断变量默认 i64 的占位被 callee 返回类型覆盖，`let v = obj.method()` 别名跟随调用结果类型）；③ **MIR Never 分支 phi 语义**（`lower_loop` 无 break 无限循环不产生块值 + `LoopCtx.had_break` 追踪 + `lower_if` 两分支均无值时返回 None）——`loop {}` 充当 panic 不再污染 if/phi 合并类型，解锁**泛型方法返回聚合类型**（`Option<Result<i64,i64>>` 嵌套单态化） |
| P009 | ✅ 完成 | 2026-08-20 | 6/6 (100%) | 标准库核心类型 + **编译器标准库搜索路径接入**：纯 Zeta 实现 `Option<T>`/`Result<T, E>`（泛型 enum + 泛型 impl + match + `loop {}` 充当 panic/Never）沉淀于 `zeta-std/zeta/core.zeta`；driver 文件入口 API 自动注入（`stdlib.rs` 定位：`ZETA_STD_PATH` 环境变量优先/仓库布局兜底，`--no-std` 禁用，组合源码哈希纳入增量缓存键）；测试 `std_test.rs` 4 用例（字符串 API 内联）+ `std_prelude_test.rs` 2 用例（文件 API 自动注入：Option/Result 直接可用 + 嵌套泛型单态化）。已有基础：NIO/sendfile 绑定层 + tests/nio_test.rs |
| — | ✅ 完成 | 2026-08-20 | 9/9 (100%) | **for 循环 range 迭代器**：typecheck 层 desugar 为 `loop`（临时边界变量 + mutable 迭代变量初始 `start-1`；loop 体首句 `pat += 1`（continue 回跳也执行 → 不会跳过递增）；次句退出判断 `if pat >= hi { break; }`（上界闭区间为 `>`）；`lower_inclusive=false` 时 `start=lo+1`；唯一临时名 `__for_lo_N`/`__for_hi_N`；循环后清理符号表）；**修复 continue 死循环**（初版 desugar 为 `while` + 末尾递增，continue 跳过递增导致挂起，`for_break_continue` 复现）；覆盖半开/闭/左开右闭区间、break/continue、嵌套 for、变量边界、下降 range 空循环、非 range 迭代器报错（双开区间 `0<..<5` 非语法，等价用 `1..<5`） |
| — | ✅ 完成 | 2026-08-20 | 11/11 (100%) | **跨模块路径表达式**：表达式支持多段路径 `mod::Enum::Variant`（`check_expr` Path/Call 分支改用 `rsplit_once` 右侧拆分 helper `split_variant_path`，2 段/3 段/裸变体统一）+ **模块常量引用** `mod::CONST`（`lookup_constant` 查找，常量可参与表达式运算）+ 模块函数（已有）；**match 模式多段路径**（AST 新增 `AstPattern::EnumPath`，parser 收集路径段（修复漏 push 末段 bug）、typecheck 取末段复用 `Enum` 逻辑）；覆盖无参/带参变体、嵌套模块、跨模块泛型 enum + match、裸变体回归、常量未定义报错 |
| — | ✅ 完成 | 2026-08-20 | 12/12 (100%) | **索引访问 `a[i]` 与数组字面量**：① 数组字面量 `[a, b, c]`（AST `ExprKind::ArrayLit` + parser `parse_array_lit` + typecheck 展开为 `Alloc + 逐元素 FieldSet` 块，数组值为槽区指针；元素类型统一校验；空数组/非字面量大小报错）；② **索引读取** `arr[i]`/`s[i]`（HIR `Index` → MIR/LIR `IndexGet` → LLVM GEP+bitcast+load；数组元素步长 8 字节、字符串字符步长 1 字节（`is_str` 标志）；索引须整数）；③ **索引写入** `arr[i] = v`（Assign 目标 `Index` → HIR `IndexSet` → `IndexSet` 指令 GEP+store，仅纯赋值）；④ **类型系统修复**：`resolve_ast_type` 保留数组长度 `[T; N]`（原忽略为 0）；⑤ 覆盖嵌套数组链式索引 `m[1][0]`、结构体字段数组 `g.rows[1]=99`、函数参数数组 + for 遍历、数组别名共享堆数据语义、compile-fail（非数组/非整数索引、元素类型不一致、空数组）——M2.5 标准库 collections 硬前提 |
| P010 | ✅ 完成 | 2026-08-20 | 28/28 (100%) | 智能区域（`SmartRegion`：静态大小推断 + PGO 画像/推荐 + EWMA 自适应扩容 + 碎片/事件统计 + 编译器集成报告 + criterion 基准）；补齐 M2.8 分配器侧 |
| P011 | ✅ 完成 | 2026-08-20 | 18/18 (100%) | MIR CFG lowering + 常量折叠/DCE/内联；修复 new_block 终止符错位 bug |
| P012 | ✅ 完成 | 2026-08-20 | 16/16 (100%) | L0 借用检查（use-after-move + 不可变赋值）；补齐 M1.5 |
| P013 | ✅ 完成 | 2026-08-20 | 18/18 (100%) | LIR 三地址码 + LLVM IR 文本 + driver 端到端（clang 汇编/链接/运行）；补齐 M1.8/M1.9 |

**状态标记**：⏳ 待开始 | 🔧 进行中 | ✅ 完成 | ❌ 阻塞

---

## 通用约定

所有 Prompt 共享以下约定：

### 代码风格

- Rust 2021 edition
- `cargo fmt` 格式化
- `cargo clippy -- -D warnings` 零警告
- 公共 API 必须有 `///` 文档注释
- 错误类型使用 `thiserror`

### 测试要求

- 每个公开函数至少一个单元测试
- 每个模块至少一个集成测试
- 性能敏感模块必须有 `criterion` 基准测试
- 解析器必须有 `libfuzzer` 模糊测试

### 文件头模板

```rust
//! # zeta-xxx
//!
//! 简要描述本模块的功能。
//!
//! ## 使用示例
//!
//! ```rust
//! // 示例代码
//! ```

#![warn(missing_docs)]
#![warn(unsafe_code)]
```

---

## 项目文档导航

| 文档 | 路径 | 说明 |
|------|------|------|
| 项目总纲 | [../CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 语法规范 | [../docs/grammar.md](../docs/grammar.md) | EBNF 语法 |
| 语义规则 | [../docs/semantics.md](../docs/semantics.md) | 类型/求值规则 |
| 内存模型 | [../docs/memory-model.md](../docs/memory-model.md) | 分层内存 |
| Actor 模型 | [../docs/actor-model.md](../docs/actor-model.md) | 并发规范 |
| 标准库 API | [../docs/std-lib.md](../docs/std-lib.md) | API 规范 |

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
