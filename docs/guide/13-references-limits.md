# 13. 参考与已知限制

## 13.1 权威文档（实现细节与语法精确定义）

| 文档 | 内容 |
|------|------|
| [grammar.md](../grammar.md) | 完整语法规范（EBNF） |
| [semantics.md](../semantics.md) | 语义规则 |
| [memory-model.md](../memory-model.md) | 分层内存管理规范 |
| [actor-model.md](../actor-model.md) | Actor 并发模型规范 |
| [module-system.md](../module-system.md) | 模块系统规范 |
| [std-lib.md](../std-lib.md) | 标准库 API 规范（含规划中模块） |
| [development-plan.md](../development-plan.md) | 开发计划（阶段 A–Z 已全部完成） |

## 13.2 工具链命令速查

| 命令 | 功能 |
|------|------|
| `rlyeh new <name> [--lib]` | 创建项目脚手架 |
| `rlyeh build <file> [-o <out>] [--target <triple>]` | 编译为可执行文件（`--target` 交叉编译 / `wasm32-wasip1` 生成 `.wasm`） |
| `rlyeh run <file>` | 编译并运行 |
| `rlyeh test` | 运行测试目录用例 |
| `rlyeh fmt <file>` | 代码格式化（`--check` / `-w` / `--indent`） |
| `rlyeh check <file>` | 静态分析 |
| `rlyeh bench <file>` | 基准测试（`--runs` / `--warmup`） |
| `rlyeh doc <file>` | 从 `///` 注释生成文档 |
| `rlyeh publish [--registry] [--verbose]` | 打包发布到 dagon 注册表（重复版本拦截） |
| `rlyeh lsp` | 语言服务器（LSP over stdio，诊断推送） |
| `rlyeh profile <file.rl_profile>` | PGO 画像 → 区域大小预测报告（`--out`） |

## 13.3 MVP 已知限制（规划中特性）

以下语法可解析但 MVP **未实现**（typecheck 显式报 Unsupported），编写时务必规避：

| 特性 | 状态 / 说明 |
|------|------|
| 宏调用 | ✅ 已实现（`macro_rules!`、`println!`、`vec!`/`map!`/`arr!`、`r#"..."#` 原始字符串） |
| 引用类型 | ✅ 已实现（`&x`/`&mut x`、`&T`/`&mut T`、`*` 解引用、`ref`/`ref mut` 模式、严格借用检查、`&str` 只读视图、`str` 值一等类型、字面量实参自动升级） |
| 闭包 | ✅ H2 无捕获 / H3 捕获闭包（IIFE）/ H5 闭包值对象；**跨线程闭包已支持（F-M2/F-M3/F-M4，0.2.0-F）**：`Thread::start(move || ..)` 将 `move` 闭包（捕获拥有环境）跨线程执行，捕获类型须满足 `'static`（禁止捕获借用引用）；无捕获闭包值仍可经 fn 签名降级为 fn 指针跨边界。限制：按引用捕获规划中；一般「闭包作 fn 实参/返回值」的闭包参数类型语法仍规划中 |
| 函数指针 | ✅ 已实现（H1） |
| 运算符 | ✅ `?`（K1）、`as` 转换（U6）、`dyn Trait`（H4，非泛型 trait/impl、含 Self 签名方法不可经 dyn 调用） |
| 所有权层级 | ✅ K2 `Box`/K3 `Rc`/`Arc`/K4 `Gc` |
| 并发 | ✅ Actor `async`/`.await`/交叉编译 WASM（L4）；普通 `async fn`/`.await` 已支持（S1c/W1–W5） |
| 表达式 | ✅ 比较链、集合/区间 `in`、`if` 表达式、块表达式尾值、`loop`/`while`/`for`、赋值表达式值 |
| 字符串 | ✅ 字面量/类型转换/`from` 构造/拼接/比较/子串/索引/`push_str`/`clone`/`as_str` 升级；`to_upper`/`to_lower`/`trim`/`split`/`replace`/`repeat`/`chars`/`lines`/`starts_with`/`contains`/`strip_prefix`/`strip_suffix`/`truncate`/`int_to_string`/`string_to_int`；`char` 升级为 32 位 Unicode 码点 |
| 切片 | ✅ 动态切片 `v[lo..<hi]`（值拷贝）、切片引用 `&[T]`/`&mut [T]`（S1/S2/S3） |
| 数组/Vec | ✅ 高阶函数 `map`/`filter`/`fold`/`collect`/`take`/`skip`（数组+Vec）、`Vec::iter`/`iter_mut`（V1）、`String::chars`/`lines`（V2） |
| 模块 | ✅ 顶层 `module`、别名导入+重命名、多文件模块、跨模块路径；不支持 `pub use`/相对 `super`/嵌套模块（规划中） |
| FFI | ✅ 低阶 `extern "C"` + 混合构建；高阶绑定规划中 |

> 规范文档（`grammar.md`/`semantics.md` 等）仍保留「规划」标注，仅表示长期路线图；本指南与 [manual/std](../manual/std/index.md) 只描述 MVP 已实现项。

---

## 练习

1. 打开 `docs/grammar.md` 与 `docs/semantics.md`，对照 [`examples/by-chapter/03-basic-syntax.rl`](../../examples/by-chapter/03-basic-syntax.rl) 验证语法规则。
2. 故意写一个 MVP 未支持的特性（如嵌套模块 `module a { module b {} }`），看 typecheck 报什么错。
3. 用 `rlyeh check` 扫描一个文件，列出它发现的"未使用变量 / 不可达代码 / 恒常条件"等告警类型。

---

[← 上一章：外部函数接口](./12-ffi.md) | [返回指南目录](./index.md)
