# 阶段 F — 编译器深度

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-a-f.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| F1 | LSP 服务器（M2.4）：IDE 支持 | ✅ 完成（MVP：新 crate `rlyeh-lsp`，JSON-RPC over stdio + full 文档同步 + 诊断推送；`rlyeh lsp` 命令 | [`f1-lsp.md`](../tasks/leaf/f1-lsp.md) |
| F2 | PGO 数据回灌编译流程（M2.8 后半）：`.rl_profile` → 区域大小预测 | ✅ 完成（`rlyeh profile` 命令 + `rlyeh build --profile` 编译期注入：加载画像 → PgoAdvisor p95×1.1 建议 → CompilerInterface 报告；语言级 region 接线后可回灌 `region 'r adaptive` 初始容量 | [`f2-pgo.md`](../tasks/leaf/f2-pgo.md) |
