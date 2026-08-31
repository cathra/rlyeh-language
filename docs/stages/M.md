# 阶段 M — 错误处理基底

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-m-t.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：`Option`/`Result` + `expect`/`unwrap_or` 已实现（K1 `?` 已支持）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。
> 目标：建立贯穿全部 std 模块的错误体系，为 File/fs/TCP 对象化提供前置基底。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| M1A | **`IoErrorKind` 枚举**：`enum IoErrorKind { NotFound, PermissionDenied, AlreadyExists, InvalidInput, WouldBlock, TimedOut, Other }`（复用 enum/match ✅，独立可验收） | ✅ 已完成 | [`m1a-ioerrorkind.md`](../tasks/leaf/m1a-ioerrorkind.md) |
| M1B | **`IoError` 结构**：`struct IoError { kind, message: String }` + 构造器/`kind()`/`message()` 访问器（正式 `Display` trait 随 Q3，MVP 先用 `message()`） | ✅ 已完成 | [`m1b-ioerror.md`](../tasks/leaf/m1b-ioerror.md) |
| M2A | **`Error` trait**：`trait Error { fn message(&self) -> String; }` + `impl Error for IoError` + `describe(e: &dyn Error)` 验证（H4 dyn Trait ✅） | ✅ 已完成 | [`m2a-error-trait.md`](../tasks/leaf/m2a-error-trait.md) |
| M2B | **错误转换约定**（`From`/`Into` 泛型 trait 验证）：验证结论——泛型 trait 声明可解析（`trait From<T>`），但 trait 方法返回 `Self` 未支持（typecheck `undefined type Self`），且 parser 无 where 子句（blanket impl `impl<T, U> Into<U> for T where U: From<T>` 不可行）。**MVP 回退**：`IoError::from_kind(kind)` 窄化转换入口（kind → 默认 message），语义等同 `From::from` | 已完成 | [`m2b-error-convert.md`](../tasks/leaf/m2b-error-convert.md) |
| M3A | **io 自由函数 Result 化**：`read_file`/`write_file`/`append_file`/`read_line` 从"空串/-1"升级为 `Result<T, IoError>`（io.rl + 绑定层返回码映射） | ✅ 已完成 | [`m3a-io-result.md`](../tasks/leaf/m3a-io-result.md) |
| M3B | **net 自由函数 Result 化**：`tcp_connect`/`send_all`/`recv_some`/`hostname` 同步升级（net.rl） | ✅ 已完成 | [`m3b-net-result.md`](../tasks/leaf/m3b-net-result.md) |
| M3C | **测试与示例迁移**：破坏性变更——同步更新全部受影响的 run-pass/compile-fail 用例与 guide/index.md/std-lib.md 示例；`?` 运算符在 std 内部使用 | ✅ 已完成 | [`m3c-test-migrate.md`](../tasks/leaf/m3c-test-migrate.md) |

**验收**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) 各子任务叶子的「验证」字段；全量回归通过。
