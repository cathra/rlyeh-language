# 阶段 X — 序列化/格式化/时间完整化

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-u-z.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：Q 阶段已实现 json 内建 + Serialize protocol（退化）+ TOML 轻量 MVP + `Display`/`Debug`（签名降级）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| X1 | **时间 API 完整**：`Duration` 补 `microseconds`/`nanoseconds` 构造器、`as_secs`/`as_millis`/`as_nanos`（u128 返回——`as_nanos` 可退化为 i64 注记）、布局对齐目标 `nanos: u64`（内部 micros: i64 迁移）；`Instant::duration_since(&self, earlier)`；新增 `time/system.rl`：`SystemTime`（墙钟纪元，`UNIX_EPOCH`/`now`/`duration_since`，`__rlyeh_clock_realtime` 注入，Windows 差异屏蔽） | ✅ 已完成 | [`x1-time-api.md`](../tasks/leaf/x1-time-api.md) |
| X2 | **标准 TOML + 解析鲁棒性**：`toml::to_string` 输出标准形式——`key = value` 空格、`[section]` 行式子表（替代内联表 `{...}`）、`#` 注释、多行字符串（`"""`）；`toml::from_str` 接受注释/空格/子表；修复 §9.4 已知限制——值含逗号/嵌套容器分段不可靠（替换 `split(",")` 为逐字符/引号感知解析）、`[T; N]` 数组反序列化、f64 支持 | ✅ 已完成（`key = value` 空格 + round-trip + 整行注释 + 引号感知 + `[section]` 行式（含多级 `[a.b]`）+ f64 + `[T; N]` 数组 + 多行字符串 `"""`，2026-08-30） | [`x2-standard-toml.md`](../tasks/leaf/x2-standard-toml.md) |
| X3 | **`Deserialize` protocol + Serializer/Deserializer 框架**：`protocol Deserialize { fn from_json(s: String) -> Self; }`（依赖 U4 `-> Self`，替换「解析统一走内建」退化）；`Serializer`/`Deserializer` 访问器框架（目标签名 `serialize(&self, &mut Serializer) -> Result<(), SerError>`，U3 约束泛型化 `to_string<T: Serialize>` 落地）；`JsonError`/`TomlError` 类型（§12，替代非 Result 包装/死循环语义）；`json::parse` 非法输入改返回 `Err` | ✅ 已完成（`Deserialize` protocol + `JsonError`/`TomlError` + `Serializer`/`Deserializer` 访问器框架 ✅；`json::try_parse`/`toml::try_parse` Err 路径 ✅，2026-08-30） | [`x3-deserialize-protocol.md`](../tasks/leaf/x3-deserialize-protocol.md) |
| X4 | **Formatter 完整化**：`Formatter` 从占位类型升级为真实格式化器（对齐/宽度/精度/填充状态字段）；`protocol Display { fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }`（目标签名，依赖 U3/U4，替换 `-> String` 降级；`Debug::fmt_debug` 改名对齐 `Debug::fmt`——同名冲突经 impl 方法符号带 protocol 区分 + MethodCall `protocol_hint` 分派消除）；`FmtError` 类型 | ✅ 已完成（Formatter 升级 + `Result<(),FmtError>` 签名 + `Debug::fmt` 改名 + `FmtError` + 对齐占位符引擎应用 ✅，2026-08-28） | [`x4-formatter-complete.md`](../tasks/leaf/x4-formatter-complete.md) |

**验收**：见任务树 [`stage-u-z.md`](../tasks/stage-u-z.md) 各子任务叶子的「验证」字段；全量回归通过。
