# docs/design — 早期设计稿（已归档）

> 本目录存放 Zeta 项目 **bootstrap 阶段（2026-07 前后）** 的设计文档。
> 这些文档是编译器从零起步时的蓝图，随开发推进已被 `docs/` 目录下的权威规范取代。
> **新功能开发以 `docs/` 与 `CODEBUDDY.md` 为准**，本目录仅作历史参考。

## 映射关系

| 归档设计稿 | 权威规范 | 实现任务 |
|------------|----------|----------|
| [00_项目总览](./00_项目总览.md) | [`CODEBUDDY.md`](../../CODEBUDDY.md) / [development-plan](../development-plan.md) | — |
| [01_词法分析器](./01_词法分析器.md) | [grammar.md](../grammar.md)（词法 EBNF） | [P001](./prompts/P001_词法分析器核心.md) |
| [02_语法分析器](./02_语法分析器.md) | [grammar.md](../grammar.md)（语法 EBNF） | [P002](./prompts/P002_语法分析器核心.md) |
| [03_类型系统](./03_类型系统.md) | [semantics.md](../semantics.md) | [P003](./prompts/P003_比较链语义分析.md) |
| [04_所有权与借用检查器](./04_所有权与借用检查器.md) | [semantics.md](../semantics.md) / [memory-model.md](../memory-model.md)（L0 层） | [P012](./prompts/P012_L0借用检查器实现.md) |
| [05_区域内存管理系统](./05_区域内存管理系统.md) | [memory-model.md](../memory-model.md)（L1 层） | [P004](./prompts/P004_区域系统实现.md) / [P005](./prompts/P005_Transfer语义实现.md) / [P010](./prompts/P010_智能区域分配器.md) |
| [06_比较链与条件判断](./06_比较链与条件判断.md) | [grammar.md](../grammar.md) / [semantics.md](../semantics.md) | [P003](./prompts/P003_比较链语义分析.md) |
| [07_Actor并发模型](./07_Actor并发模型.md) | [actor-model.md](../actor-model.md) | [P006](./prompts/P006_Actor运行时.md) |
| [08_编译器后端与代码生成](./08_编译器后端与代码生成.md) | [CODEBUDDY.md](../../CODEBUDDY.md) §4 / [development-plan](../development-plan.md) | [P011](./prompts/P011_MIR中间表示实现.md) / [P013](./prompts/P013_LLVM后端与代码生成.md) |
| [09_工具链设计](./09_工具链设计.md) | [CODEBUDDY.md](../../CODEBUDDY.md) §5 | [P007](./prompts/P007_增量编译引擎.md) / [P008](./prompts/P008_包管理器Zep.md) |
| [10_标准库规划](./10_标准库规划.md) | [std-lib.md](../std-lib.md) | [P009](./prompts/P009_标准库核心模块.md) |

## 说明

- **2026-08 归档**：设计稿由 `zeta-language/` 根目录移入本目录（`git mv`，历史保留）。
- 各设计稿头部已加归档横幅；正文内容保持原样，**不再更新**。
- **2026-08-24 任务书归档**：开发任务书（原 `prompts/P001–P013`）随其内容并入
  `docs/` 权威文档（各文档"附录 A：实现纪要"）后，一并归档至本目录 [`prompts/`](./prompts/README.md)。
- 权威进度文档：`docs/development-plan.md`（阶段 A–F 执行记录，已完成）+ `docs/mvp-gaps-plan.md`（阶段 G–T 剩余任务消解计划，已完成）。
