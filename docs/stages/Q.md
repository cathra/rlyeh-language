# 阶段 Q — 序列化与格式化 trait

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-m-t.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：`json::stringify`/`json::parse::<T>` 编译器内建已实现（L2 ✅，含 HashMap，turbofish 泛型实参）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| Q1A | **`Serialize`/`Deserialize` trait 定义**：`trait Serialize { fn to_json(&self) -> String; }` + `trait Deserialize { fn from_json(s: String) -> Self; }`（**风险验证结论**：`-> Self` 返回自身类型未支持——typecheck `undefined type Self`，见 M2b） + 内建类型（标量/数组/Vec/HashMap/String）默认 impl | ✅ 已完成 | [`q1a-serde-trait.md`](../tasks/leaf/q1a-serde-trait.md) |
| Q1B | **derive 宏语法**：`#[derive(Serialize, Deserialize)]` 解析（lexer 新增 `Pound` token + parser `parse_attributes` 特判，`AstStructDecl.derive` 存储 trait 名列表；其它 attribute 名报错、非 struct 项宽松忽略） | ✅ 已完成 | [`q1b-derive-macro.md`](../tasks/leaf/q1b-derive-macro.md) |
| Q1C | **derive 生成接线**：struct 派生 impl——序列化复用 L2 内建 stringify（字段序 = 定义序）；反序列化 `json_parse_ast` 新增 struct 分支（desugar 为块表达式：`String::from` → `substring` 剥离 `{}` → `split(",")` → `for` 遍历 → 字段名匹配 if-else 链逐字段 `Assign`，值递归 `json_parse_ast`；字段顺序任意、缺失字段保持零值、未知字段忽略、嵌套 struct 支持；嵌套 struct/Vec/HashMap 值含逗号经 split 分段错误的 MVP 限制；泛型 struct 不支持；空 struct 报 Unsupported） | ✅ 已完成 | [`q1c-derive-gen.md`](../tasks/leaf/q1c-derive-gen.md) |
| Q2A | **泛型 API 入口**：`json::to_string<T: Serialize>`/`from_str<T: Deserialize>`（turbofish 泛型实参；保留既有内建 `stringify`/`parse` 兼容） | ✅ 已完成 | [`q2a-generic-api.md`](../tasks/leaf/q2a-generic-api.md) |
| Q2B | **流式 writer/reader**：`to_writer`/`from_reader`（目标 File/TcpStream，N/O 完成后接线） | ✅ 已完成 | [`q2b-writer-reader.md`](../tasks/leaf/q2b-writer-reader.md) |
| Q3A | **`Display`/`Debug` trait + `Formatter` 类型**：`impl Display for T { fn fmt(&self, f: &mut Formatter) }`（`Formatter` 为 `&mut` 引用参数——G1 ✅ 已支持） | ✅ 已完成 | [`q3a-display-debug.md`](../tasks/leaf/q3a-display-debug.md) |
| Q3B | **格式化引擎接入**：`{}` 占位解析优先查 `Display` impl（内建类型保持现状，自定义类型走 trait 方法），`{:?}` 走 `Debug`（I2 收尾） | ✅ 已完成 | [`q3b-format-engine.md`](../tasks/leaf/q3b-format-engine.md) |
| Q4 | **TOML 模块**（轻量）：基础标量/嵌套表/数组 stringify/parse | ✅ 已完成 | [`q4-toml.md`](../tasks/leaf/q4-toml.md) |

**验收**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) 各子任务叶子的「验证」字段；全量回归通过。
