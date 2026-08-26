# 阶段 A–F（编译器 + 工具链）

> **所属任务树**：[任务文档导航](./README.md)
> **状态**：✅ 全部完成
> **权威来源**：阶段详情文档 [`stages/A.md`](../stages/A.md)、[`stages/F.md`](../stages/F.md)；执行记录见本文件 §执行记录
> **本层职责**：记录阶段 A–F 的任务列表与进度 + 执行记录；具体实施见权威源文档（development-plan.md）。

---

## 任务列表与进度

| 阶段 | 主题 | 子任务 | 状态 |
|------|------|--------|------|
| **A** | 编译器加固（泛型/Infer/FFI/字符串语义） | A1–A4 | ✅ 全部完成 |
| **B** | 标准库完善（time/io/net/sync/collections） | B1–B5 | ✅ 全部完成 |
| **C** | Actor 语言级接线（`actor`/`spawn`/`.await`） | C1–C3 | ✅ 全部完成 |
| **D** | 工具链（`rlyeh test`/`fmt`/`check`/`doc`/`bench`） | D1–D3 | ✅ 全部完成 |
| **E** | 多目标与发布（交叉编译/WASM/发布流程） | E1–E3 | ✅ E1 大部分完成（macOS 双架构；Windows/ARM 待环境）；E2/E3 完成 |
| **F** | 编译器深度（LSP、PGO 回灌） | F1–F2 | ✅ 全部完成 |

---

## 阶段详情索引

- 阶段 A–F 的详细任务说明：阶段详情文档 [`stages/A.md`](../stages/A.md)、[`stages/F.md`](../stages/F.md)
- 阶段 A–F 的执行记录：见本文件 §执行记录

---

## 子任务列表（按阶段分层）

> 每个阶段下列出其子任务（一个任务一个叶子文档，含具体执行情况与技术细节）：

### A — 编译器加固

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| A1 用户级 match 解构具体实例化枚举聚合载荷 | [`a1-match-payload.md`](leaf/a1-match-payload.md) | ✅ 已完成 |
| A2 Infer 枚举自动定型 | [`a2-infer-enum.md`](leaf/a2-infer-enum.md) | ✅ 已完成 |
| A3 String 拼接拷贝语义 | [`a3-string-concat.md`](leaf/a3-string-concat.md) | ✅ 已完成 |
| A4 通用 FFI `extern fn` 声明 | [`a4-extern-ffi.md`](leaf/a4-extern-ffi.md) | ✅ 已完成 |

### B — 标准库完善

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| B1 `Duration`/`Instant` 时间模块 | [`b1-time-module.md`](leaf/b1-time-module.md) | ✅ 已完成 |
| B2 io 模块（文件 IO） | [`b2-io-module.md`](leaf/b2-io-module.md) | ✅ 已完成 |
| 位运算全链路（B3 前置） | [`b3-bitwise.md`](leaf/b3-bitwise.md) | ✅ 已完成 |
| B3 net 模块 | [`b3-net-module.md`](leaf/b3-net-module.md) | ✅ 已完成 |
| B4 sync 模块 | [`b4-sync-module.md`](leaf/b4-sync-module.md) | ✅ 已完成 |
| B5 Vec/HashMap 方法补齐 | [`b5-vec-hashmap-methods.md`](leaf/b5-vec-hashmap-methods.md) | ✅ 已完成 |

### C — Actor 语言级接线

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| C1 actor desugar 全链路 | [`c1-actor-desugar.md`](leaf/c1-actor-desugar.md) | ✅ 已完成 |
| C2 语言级受监督 spawn | [`c2-supervised-spawn.md`](leaf/c2-supervised-spawn.md) | ✅ 已完成 |
| C3 示例与测试固化 | [`c3-actor-examples.md`](leaf/c3-actor-examples.md) | ✅ 已完成 |

### D — 工具链

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| D1 `rlyeh test` 子命令 | [`d1-rlyeh-test.md`](leaf/d1-rlyeh-test.md) | ✅ 已完成 |
| D2 `rlyeh fmt` 格式化器 | [`d2-rlyeh-fmt.md`](leaf/d2-rlyeh-fmt.md) | ✅ 已完成 |
| D2 `rlyeh check` 静态分析器 | [`d2-rlyeh-check.md`](leaf/d2-rlyeh-check.md) | ✅ 已完成 |
| D3 `rlyeh doc` 文档生成器 | [`d3-rlyeh-doc.md`](leaf/d3-rlyeh-doc.md) | ✅ 已完成 |
| D3 `rlyeh bench` 基准框架 | [`d3-rlyeh-bench.md`](leaf/d3-rlyeh-bench.md) | ✅ 已完成 |

### E — 多目标与发布

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| E1 `--target` 交叉编译 | [`e1-cross-compile.md`](leaf/e1-cross-compile.md) | ✅ 大部分完成 |
| E1 平台内建 `__rlyeh_target_os` | [`e1-target-os-builtin.md`](leaf/e1-target-os-builtin.md) | ✅ 已完成 |
| E2 WASM 目标支持 | [`e2-wasm-target.md`](leaf/e2-wasm-target.md) | ✅ 已完成 |
| E3 发布流程 | [`e3-publish.md`](leaf/e3-publish.md) | ✅ 已完成 |

### F — 编译器深度

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| F1 LSP 服务器（MVP） | [`f1-lsp.md`](leaf/f1-lsp.md) | ✅ 已完成 |
| F2 PGO 数据回灌 | [`f2-pgo.md`](leaf/f2-pgo.md) | ✅ 已完成 |

### 遗留 — 遗留修复

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| 遗留修复（rlyeh new / 动态切片 / String::from） | [`legacy-misc.md`](leaf/legacy-misc.md) | ✅ 已完成 |

---
## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 阶段 A–F 纳入任务树（索引，权威源 development-plan.md） |
| 2026-08-26 | 执行记录归档：搬入 development-plan §4 + CODEBUDDY §5.5（A–F）到本小节，源文档对应区替换为链接 |
