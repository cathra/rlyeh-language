# 阶段 A — 编译器加固

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-a-f.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| A1 | 用户级 match 解构具体实例化枚举聚合载荷：`check_pattern` Enum 分支合并「当前 generic_subst + `pat_ty` 类型参数 ↔ `enum_def.type_params` 新映射」，嵌套泛型 `Option<Vec<T>>` 等递归定型 | ✅ 完成 | [`a1-match-payload.md`](../tasks/leaf/a1-match-payload.md) |
| A2 | Infer 枚举自动定型：`check_method_call` 参数检查时对含 `_` 的期望类型用实参 unify 回填 subst（`unify` 新增 `Type::Infer` 分支），回填后重算签名再实例化 | ✅ 完成 | [`a2-infer-enum.md`](../tasks/leaf/a2-infer-enum.md) |
| A3 | `String` 拼接 `+` 语义评审：当前为原地追加 + 共享缓冲的别名隐患，评估改为拷贝语义（`let __s = a; __s.push_str(b)` 形态） | ✅ 完成（拷贝语义：`let __s = a.clone(); __s.push_str(b)` | [`a3-string-concat.md`](../tasks/leaf/a3-string-concat.md) |
| A4 | 通用 FFI `extern fn` 声明：打通 parser→typecheck→HIR→MIR→LIR→LLVM→链接全链路，codegen 生成 `declare` 而非 `define`，符号由链接器解析 | ✅ 完成 | [`a4-extern-ffi.md`](../tasks/leaf/a4-extern-ffi.md) |
