# 阶段 C — Actor 语言级接线

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-a-f.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| C1 | `actor` 语法解析 + 语义化，桥接 `rlyeh-actor-runtime`（ActorRef/Runtime API 已就绪） | ✅ 完成 | [`c1-actor-desugar.md`](../tasks/leaf/c1-actor-desugar.md) |
| C2 | `spawn`/`.await` 调用语法与标准库集成（`Counter::new()` → `counter.increment(10).await` 形态） | ✅ 完成 | [`c2-supervised-spawn.md`](../tasks/leaf/c2-supervised-spawn.md) |
| C3 | Actor 示例（ping-pong、Supervisor 恢复）与集成测试固化 | ✅ 完成 | [`c3-actor-examples.md`](../tasks/leaf/c3-actor-examples.md) |

执行记录（阶段 C）：

- **C1 — actor desugar 全链路** / **C2 — 语言级受监督 spawn** / **C3 — 示例与测试固化** 的详细执行情况与技术细节（含排障与修复）见任务树 [`stage-a-f.md`](../tasks/stage-a-f.md) C 阶段叶子文档：[`c1-actor-desugar.md`](../tasks/leaf/c1-actor-desugar.md)、[`c2-supervised-spawn.md`](../tasks/leaf/c2-supervised-spawn.md)、[`c3-actor-examples.md`](../tasks/leaf/c3-actor-examples.md)。
