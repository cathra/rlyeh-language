# 遗留修复（rlyeh new / 动态切片 / String::from / 阶段 A–F 遗留杂项）

> **所属阶段**：阶段 A–F
> **状态**：✅ 已完成（含登记的各主题遗留限制，见各节「遗留限制」；跨阶段待补项见「阶段 A–F 其他遗留项」）
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

集中登记阶段 A–F 的**遗留杂项**：`rlyeh new` 脚手架 + Vec/数组动态切片 + `String::from` 字面量变量三项已完成功能（附遗留限制），以及散落在 A–F 各阶段的其他遗留/待补项。

## 背景

阶段 A–F 子任务，详见阶段详情文档 [`stages/L.md`](../../stages/L.md)。本文档为 A–F 阶段的**遗留杂项收口处**：凡 A–F 阶段中「✅ 大部分完成 / 待环境 / 规划」的条目，均在此登记并标注处理建议，避免散落。

## 技术细节

### 1. `rlyeh new <name> [--lib]` 项目脚手架

**实现**：`crates/rlyeh-driver/src/main.rs`（`run_new`）+ `dagon/src/commands.rs`（`cmd_new`/`write_project_files`）+ `dagon/src/manifest.rs`（`validate_package_name`）。生成 `Rlyeh.toml`（`version: "0.1.0"`、`edition: "0.2"`）+ `src/main.rl`（`fn main() { println("Hello from {name}!") }`）或 `src/lib.rl` + `.gitignore` + `git init`。

**遗留限制**：
- **仅 `--lib` 选项**：无 `--target`/`--toolchain`；任何 `-` 开头非 `--lib` 参数直接报错（`main.rs:672`）。
- **lib 模板空壳**：`lib.rl` 只有注释、无导出函数（规划 `pub fn hello()` 未落地）。
- **无模板类型选择**：只有 main/lib 二选一；`version`/`edition` 硬编码，`description`/`license`/`authors` 全空。
- **`verbose=false` 固定**：无详细输出开关。

**处理建议**：低优先级可选增强（无语言级障碍，纯 CLI 补全）。

### 2. Vec<T>/数组动态切片

**实现**：`crates/rlyeh-typecheck/src/check_expr/field.rs`（`check_slice`）+ `index_enum.rs`（`check_index` Range 特判）。`..<` 半开 / `...` 双闭 / `<..` 不含下；String/&str → substring，Vec<T> → `Vec::slice`，数组 → 展开 Vec 拷贝循环 + clamp。

**遗留限制**：
- **仅 String/&str/Vec/数组四类**：HashMap 不可切片（`field.rs:116-121` Unsupported）。
- **无 `[..]` 省略边界**：`ExprKind::Range` 的 `lower`/`upper` 均必填，`v[..]`/`v[0..]`/`v[..<3]` 不可用（parser `expr/mod.rs` 强制两侧表达式）。全量切片须 `v[0..<N]`。
- **返回全新缓冲（值拷贝）无零拷贝视图**：数组切片逐元素 push（`alloc_array(4)` 初始容量硬编码 4）+ String substring 拷贝返回；无借用/视图语义（设计决策见 g2-str-slice.md）。
- **负索引 clamp 到 0，非 Rust 倒数语义**：`v[-3..<2]` → `[0,2)`。

**处理建议**：中等优先级；`[..]` 省略边界（Range 两侧改 `Option`）为最常见缺口。

### 3. `String::from(s)` 支持绑定字面量的变量

**实现**：`TypeContext.local_inits`（`context.rs:229-253`）+ `check_string_from`（`construct.rs:916-930`）+ `upgrade_str_arg`。`let`/`let ref` 绑定记录 init HIR；`String::from(变量)` 回查 `local_inits`，仅当查回 `HirExpr::StringLiteral` 才采用。

**遗留限制**：
- **只追踪 `let` 绑定**：函数参数、match 模式绑定、循环变量、赋值（`=`）不追踪 init 字面量。
- **只认直链字面量（一层）**：变量初始化为另一变量/表达式/拼接结果则失败，报 Unsupported（`construct.rs:918-929`）。
- **fn 边界不穿透**：`lookup_local_init` 遇 `is_fn` 层即停（`context.rs:242-252`），跨函数/闭包不可见。
- **同名遮蔽误判风险（历史已修）**：此前 `check_fn_body_with_self` 未隔离 `local_inits` 导致 std 方法体同名变量覆盖调用方绑定，补 `saved_inits` take/恢复修复（g2-str-value.md）。

**处理建议**：低优先级；现有覆盖已满足常用场景，扩展多层/跨函数追踪收益有限。

### 4.（新增）`dyn Trait` 作函数参数/返回值受限（源自 H4/F）

**现状**：H4 MVP 曾限制 `dyn Trait` 仅支持 `let d: dyn Trait = &obj;` 局部变量主路径。现 `&dyn Trait`（2 槽胖指针）作函数/方法参数与返回值已可用（见 lang-defects #2 / P4 `coerce_to_dyn`）：`fn source(&self) -> Option<&dyn Error>`、`fn f(x: &dyn Trait)` 等均正常，`p4_dyn_upshift.rl` / `error_source.rl` 已验证。裸 `dyn Trait`（按值，DST 不可存储）仍不可作参数/返回值，与 Rust 一致，非缺陷。

**处理建议**：已随 lang-defects #2（P4 `&dyn Error` 上转型）解决；此处保留 H4 原始限制记录。

### 5.（新增）空数组字面量 `[]` 与范围/元组模式（源自 J/B）

**现状**：`index_enum.rs`——空数组字面量 `[]` MVP 不支持；元组/结构体模式、范围模式 MVP 不支持。

**处理建议**：低优先级；空 `[]` 可用 `Vec::new()` 替代，模式受限属已知 MVP 退化。

## 阶段 A–F 其他遗留项（跨阶段待补清单）

| 项 | 来源 | 现状 | 处理建议 |
|----|------|------|---------|
| E1 交叉编译 **Windows/ARM 链接器与库路径待补** | `e1-cross-compile.md`（✅ 大部分完成）+ `stages/E.md` | macOS 双架构已通；Windows/ARM 未打通 | 已另见发布专项（toolchains 交叉编译，含 Windows arm64/Linux arm64 缺失） |
| F1 LSP **仅 full 文本同步 + 诊断推送**（MVP） | `f1-lsp.md` | JSON-RPC + full sync；无悬停/补全/跳转定义 | 低优先级；诊断为主已满足 MVP |
| B3 net「NIO/sendfile 绑定层接线随阶段 C/D 延后」 | `stages/B.md` | 延后项 | 已由 Y2 NIO 后端承接 |
| B4 sync「Condvar/Barrier 骨架留注释（待函数指针/线程创建）」 | `stages/B.md` | 骨架 | 已由 P3/C 阶段承接（condvar/barrier 已实现） |
| L2 serde「自定义 trait/derive 仍规划」 | `stages/L.md` | 规划 | 已由 Q 阶段（q1a/q1b/q1c derive）承接 |

## 验证

`dynamic_slice_test.rs` 7 用例 + 全量回归 111 套件全绿（原 A–F 收口时）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
| 2026-08-28 | 细化：三主题补充实现文件与「遗留限制」细目（rlyeh new 缺 --target/lib 空壳；切片缺 [..]/HashMap/负索引；String::from 仅 let+直链+fn 边界）；新增 #4 dyn Trait 参数限制、#5 空数组字面量/模式限制；新增「阶段 A–F 其他遗留项」跨阶段待补表（E1 Windows/ARM、F1 LSP MVP、B3/B4/L2 延后承接项） |
| 2026-09-06 | #4 `dyn Trait` 作参数/返回值标记已随 lang-defects #2（P4 `coerce_to_dyn`）解决：`&dyn Trait` 胖指针作函数/方法参数与返回值已可用（error_source.rl/p4_dyn_upshift.rl 验证）；裸 `dyn Trait` 按值不可存储与 Rust 一致 |
