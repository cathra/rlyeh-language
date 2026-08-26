# 阶段 E — 多目标与发布

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| E1 | 交叉编译：macOS / Windows / ARM 目标（消除平台相关假设） | 🔧 大部分完成（`--target` 注入 clang + 双架构验证 + 平台内建消除 `sockaddr_in4` 布局假设；Windows/ARM 链接器与库路径待补 | [`e1-target-os-builtin.md`](../tasks/leaf/e1-target-os-builtin.md) |
| E2 | WASM 目标支持（M2.7） | ✅ 完成 | [`e2-wasm-target.md`](../tasks/leaf/e2-wasm-target.md) |
| E3 | 发布流程：`dagon` 注册表版本信息、CI 多平台产物（`.github/workflows/release.yml`） | ✅ 完成（`dagon publish` 重复版本保护 + `rlyeh publish` CLI + release.yml 四平台矩阵 + `CHANGELOG.md` v0.1.0 | [`e3-publish.md`](../tasks/leaf/e3-publish.md) |
