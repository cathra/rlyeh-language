# 任务文档导航（树形）

> 按**树形结构**组织任务文档：**上层文档只记录下一级任务文档的任务列表与实现进度**，**叶子文档包含最小粒度单个任务的具体实施情况**。
>
> 结构：
> - `README.md`（本文件）= **树根**：列里程碑总览 + 各阶段索引文档 + 进度。
> - `milestones.md` = 里程碑总览（整合 A–Z 各阶段进度）。
> - `stage-*.md` = **阶段索引**：列该阶段任务列表 + 进度 + **子任务叶子文档清单**。
> - `v2-str-view.md` / `v3-iterator-adapters.md` = **任务索引**（V 阶段）：列子任务叶子文档 + 进度。
> - `leaf/` = **叶子层**：**每个最小粒度任务一个文档**（目标/技术细节/验证/状态/变更记录），全部阶段（A–Z）的任务均已细化到叶子（共 178 个）。

---

## 树根索引

| 层级 | 文档 | 内容 |
|------|------|------|
| 里程碑列表 | [`CODEBUDDY.md`](../../CODEBUDDY.md) §6 | 3 个里程碑总览（列表） |
| 里程碑任务列表 | [`milestone-1.md`](./milestone-1.md) · [`milestone-2.md`](./milestone-2.md) · [`milestone-3.md`](./milestone-3.md) | M1/M2/M3 各里程碑细化后的任务列表 |
| 里程碑单任务文档 | [`milestone-tasks/`](./milestone-tasks/) | 每个里程碑任务一个文档（`m1-1.md`…`m3-6.md`，含执行情况/技术细节） |
| 阶段 A–F | [`stage-a-f.md`](./stage-a-f.md) | 编译器 + 工具链（development-plan）；子任务列表 + 叶子 + 进度 |
| 阶段 G–L | [`stage-g-l.md`](./stage-g-l.md) | 编译器能力补齐（development-plan §3）；子任务列表 + 叶子 + 进度 |
| 阶段 M–T | [`stage-m-t.md`](./stage-m-t.md) | 标准库深度完善（development-plan §3b）；子任务列表 + 叶子 + 进度 |
| 阶段 U–Z | [`stage-u-z.md`](./stage-u-z.md) | 目标 API 对齐（development-plan §3c）；子任务列表 + 叶子 + 进度 |
| V2 任务 | [`v2-str-view.md`](./v2-str-view.md) | `&str` 引用视图（V2-A~E 子任务 + 叶子） |
| V3 任务 | [`v3-iterator-adapters.md`](./v3-iterator-adapters.md) | Iterator 适配器（V3-A1~A4/B/C/D1~D5/E 子任务 + 叶子；高风险 A/D 已分解为低/中风险） |
| 专项开发计划 | [`专项开发计划.md`](./专项开发计划.md) | 整合 lang-defects/parser-rework/legacy-misc 待办为 P1–P10，按风险/依赖排 7 批次执行序 |
| 类型系统增强（规划） | [`slice-type-system.md`](./slice-type-system.md) · [`type-union.md`](./type-union.md) | 切片类型系统（`&[T]`/`&mut [T]` 胖指针）+ 受限制的类型联合（enum 体系增强）；叶子见 `leaf/slice-*` / `leaf/union-*` |
| 自举能力缺口（P0/P1/P2） | [`self-hosting.md`](./self-hosting.md) · [`self-hosting-p0.md`](./self-hosting-p0.md) · [`self-hosting-p1.md`](./self-hosting-p1.md) · [`self-hosting-p2.md`](./self-hosting-p2.md) | 自举可行性缺口任务树（根索引 + P0/P1/P2 分级索引 + `leaf/sh-*` 叶子）；对应 [`../self-hosting/feasibility.md`](../self-hosting/feasibility.md) 与 [`../development-plan-0.2.0.md`](../development-plan-0.2.0.md) |

---

## 总进度

| 阶段组 | 状态 |
|--------|------|
| A–F（编译器 + 工具链） | ✅ 全部完成 |
| G–L（编译器能力补齐） | ✅ 全部完成 |
| M–T（标准库深度完善） | ✅ 全部完成 |
| U（编译器地基） | ✅ 全部完成 |
| **V**（集合与迭代器完整化） | ✅ **已完成**（V1/V2/V3/V4/V5 全部完成；V3 数组/`Vec` 适配器因数组非命名类型保留内建 desugar，记为已知语言限制） |
| W（异步运行时完整化） | ✅ 全部完成（2026-08-30 验收：W1–W6 集成测试 25 用例 + 全量 .rl 套件全绿；async_if_await/async_neg poll 死循环已于 2026-08-29 修复并解除 skip） |
| X（序列化/时间完整化） | ✅ 全部完成（X1/X2/X3/X4 全部完成，2026-08-30） |
| Y（收尾） | 🔧 部分完成（Y2/Y5/Y6/Y7/Y8 ✅；Y1/Y3/Y4 部分） |
| 自举缺口（P0/P1/P2） | ⏳ 规划中（P0/P1/P2 能力均入 0.2.0；P2-1 外部 crate 长期保留 Rust） |

**当前焦点**：Y 阶段收尾（Y2/Y5/Y6/Y7/Y8 ✅；Y1/Y3/Y4 部分完成）。W/X 已 ✅ 完成；V 已完成。

**叶子文档规模**：A–Z 全部任务已细化为单文档叶子（`leaf/`，共 178 个）——阶段 A–F 25 个、G–L 25 个、M–T 59 个、U–Z 49 个（含 V2/V3 各子任务）为已规划阶段子任务；另含切片/联合规划叶子与专项开发计划叶子（P1–P10 等）等补充文档，一并归档于 `leaf/`（各阶段索引 §子任务叶子文档列出）。

---

## 相关权威源

- 阶段总计划：[`development-plan.md`](../development-plan.md)（§2 总览 A–Z + §3 阶段详情索引 A–L + §6.3b/§6.3c M–Z）
- 阶段详情：[`stages/`](../stages/)（每阶段一个文档，任务表「详情」列链接到本树 `leaf/` 叶子）
- 里程碑变更记录（权威）：[`CODEBUDDY.md`](../../CODEBUDDY.md) §5/§6
- 阶段任务执行记录：见各 `stage-*.md` §执行记录（本树）

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 任务文档树形重组（V2/V3 索引 + 叶子） |
| 2026-08-26 | 整合里程碑与阶段（A–Z）到树根（milestones + stage-a-f/g-l/m-t/u-z） |
| 2026-08-26 | 全量细化：A–Z 全部任务细化为单文档叶子（155 个），stage 索引改为叶子清单 |
| 2026-08-26 | 补齐层级：stage 索引补「阶段 → 子任务列表 → 叶子」分层；里程碑执行情况迁移至 milestone-1/2/3.md |
| 2026-08-26 | 里程碑三级结构：CODEBUDDY §6 = 里程碑列表；milestone-*.md = 任务列表；milestone-tasks/ = 单任务文档（23 个） |
| 2026-08-28 | 新增专项开发计划索引（整合 lang-defects/parser-rework/legacy-misc 待办为 P1–P10 执行计划） |
| 2026-09-01 | 新增自举能力缺口任务树（P0/P1/P2 三级：根索引 + 分级索引 + `leaf/sh-*` 9 叶子），对应 `../self-hosting/feasibility.md` 与 `../development-plan-0.2.0.md` |
| 2026-09-01 | 修正版本边界（P0 上移 0.2.0 必须项）+ 新增 P0-4 并发原语、P2-4 链接桥、P2-5 分阶段自举、P2-6 诊断、P2-7 driver 自举（共 14 叶子） |
