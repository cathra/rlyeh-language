# Rlyeh 文档导航

> **目录定位**：本目录是 Rlyeh 的**权威规范文档**与**开发进度文档**所在地（唯一真相来源）。
> 早期设计稿按模块归档于 [`design/`](design/README.md)（不再更新，仅供追溯）。
> 项目总纲（全景/工具链/特性速览）见 [`../CODEBUDDY.md`](../CODEBUDDY.md)。

---

## 1. 文档地图

| 文档 | 定位 | 读者 | 说明 |
|------|------|------|------|
| [tutorial.md](./tutorial.md) | 新手入门 | 新读者 | 安装 → 第一个程序 → 发布项目全流程 |
| [guide.md](./guide.md) | 语言教程 | 语言使用者 | 示例均可运行的渐进式教程（主文档），含 MVP 已知限制汇总 |
| [manual.md](./manual.md) | 语言参考 | 语言使用者 | 速查：词法 / 类型 / 运算符 / 标准库 API / 工具链命令 |
| [grammar.md](./grammar.md) | 语法规范 | 编译器开发者 / 使用者 | 完整 EBNF 语法（含规划标注） |
| [semantics.md](./semantics.md) | 语义规则 | 编译器开发者 | 类型 / 求值 / 名称解析规则（含规划标注） |
| [memory-model.md](./memory-model.md) | 内存模型 | 编译器开发者 / 使用者 | 分层内存管理（L0 所有权 → L3 GC）规范 |
| [actor-model.md](./actor-model.md) | 并发规范 | 编译器开发者 / 使用者 | Actor 并发模型（spawn / ask / supervisor 等） |
| [module-system.md](./module-system.md) | 模块系统规范 | 编译器开发者 / 使用者 | 模块系统设计（语法/语义/编译模型/包集成 + 演进路线） |
| [std-lib.md](./std-lib.md) | 标准库 API | 编译器开发者 / 使用者 | 各模块目标 API（✅ 已实现 / 📋 规划），MVP 差异注记 |
| [development-plan.md](./development-plan.md) | 进度计划 | 团队 | 开发计划（阶段 A–Z 总览 + 阶段详情索引；§2 总览表链接各阶段详情） |
| [stages/](./stages/) | 阶段详情 | 团队 | 每阶段一个详情文档（A–Y，含任务列表 + 各任务详情文档链接到 tasks/leaf） |
| [tasks/README.md](./tasks/README.md) | 任务文档（树形） | 团队 | 任务树根（里程碑总览 + 阶段 A–Z 索引 + 进度）；索引层 stage-* / v2-str-view / v3-iterator-adapters（含 §执行记录），叶子层 leaf/ 为最小粒度子任务具体实施 |

## 2. 建议阅读顺序

- **语言使用者**：`tutorial.md`（新手入口）→ `guide.md` →（按需）`manual.md` / `std-lib.md` → `grammar.md` / `memory-model.md`
- **编译器开发者**：`semantics.md` → `grammar.md` → `memory-model.md` → `actor-model.md` → `module-system.md` →（各文档尾部"实现纪要"附录，任务书归档于 [`../design/prompts/`](design/prompts/README.md)）
- **标准库开发者**：`std-lib.md` + `design/10_标准库规划.md`
- **团队排期**：`development-plan.md`（阶段 A–Z，当前主线）

## 3. 规范与一致性规则

1. **规范冲突以本目录为准**：`design/`（早期设计稿）与 `design/prompts/`（任务书，2026-08-24 归档）仅作追溯，如与 `docs/` 根目录规范冲突，以根目录为准。
2. **规划标注惯例**：`grammar.md` / `semantics.md` / `memory-model.md` / `actor-model.md` 中以 `<!-- 规划 -->` 标注未实现特性；`std-lib.md` / `module-system.md` 以状态标记（✅/🔧/📋）标注实现程度。
3. **进度文档衔接**：`development-plan.md`（阶段 A–Z：A–F 已完成 + §6 剩余任务消解 G–L / M–T / U–Z）为统一开发计划；新阶段任务从 §6.3c（U–Z）消解，且**必须**按 [`tasks/README.md`](./tasks/README.md)（§7.4 任务管理体系）在 `tasks/` 树建档。
4. **新增文档**：规范类 → 对应权威文档增补（不新建散文件）；任务执行记录 → `tasks/` 树（阶段索引 §执行记录 / 叶子文档）；历史归档 → `design/`。

---

> **维护者**：Rlyeh Language Team
