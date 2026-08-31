# 阶段 D — 工具链

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-a-f.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| D1 | `rlyeh test`：接入 `tests/` 目录（compile-pass/compile-fail/run-pass）与 cargo 测试矩阵 | ✅ 完成 | [`d1-rlyeh-test.md`](../tasks/leaf/d1-rlyeh-test.md) |
| D2 | `rlyeh fmt` + `rlyeh check`（`tools/` 下 crate 已有骨架） | ✅ 完成 | [`d2-rlyeh-check.md`](../tasks/leaf/d2-rlyeh-check.md) |
| D3 | `rlyeh doc`（`///` 注释提取）+ `rlyeh bench`（criterion 基准） | ✅ 完成 | [`d3-rlyeh-bench.md`](../tasks/leaf/d3-rlyeh-bench.md) |

执行记录（阶段 D/E）：

- **D1 — `rlyeh test`** / **D2 — `rlyeh fmt` / `rlyeh check`** / **D3 — `rlyeh doc` / `rlyeh bench`** 的详细执行情况与技术细节（含测试固化与连带修复）见任务树 [`stage-a-f.md`](../tasks/stage-a-f.md) D 阶段叶子文档：[`d1-rlyeh-test.md`](../tasks/leaf/d1-rlyeh-test.md)、[`d2-rlyeh-fmt.md`](../tasks/leaf/d2-rlyeh-fmt.md)、[`d2-rlyeh-check.md`](../tasks/leaf/d2-rlyeh-check.md)、[`d3-rlyeh-doc.md`](../tasks/leaf/d3-rlyeh-doc.md)、[`d3-rlyeh-bench.md`](../tasks/leaf/d3-rlyeh-bench.md)。
- **E1 — `--target` 交叉编译 + 平台内建** 的详细执行情况与技术细节见任务树 [`stage-a-f.md`](../tasks/stage-a-f.md) E 阶段叶子文档：[`e1-cross-compile.md`](../tasks/leaf/e1-cross-compile.md)、[`e1-target-os-builtin.md`](../tasks/leaf/e1-target-os-builtin.md)。
