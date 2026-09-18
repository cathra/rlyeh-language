# 阶段 N — 文件系统与 IO 对象化

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-m-t.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：仅 4 个自由函数（`read_file`/`write_file`/`append_file`/`read_line`），无 File 对象、无 Path/fs、无 stdout/stderr。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| N1A | **`OpenMode` + Rust 绑定层**：`enum OpenMode { Read, Write, Append, ReadWrite, Create }` + 新增 `rlyeh-std/src/file.rs`（`std::fs::OpenOptions`/`File` 封装 C ABI：open/create/read/write/flush/metadata/close） | ✅ 已完成 | [`n1a-open-mode.md`](../tasks/leaf/n1a-open-mode.md) |
| N1B | **`File` 对象化**：`File::open_with(path, mode)`/`File::create` + 句柄封装 + `close`（Rlyeh 无 Drop，MVP 显式 `close()` 语义；Y1 起 `File::open(path)` 为默认只读兼容壳） | ✅ 已完成 | [`n1b-file-object.md`](../tasks/leaf/n1b-file-object.md) |
| N1C | **读写与元数据方法**：`read_to_string`/`read(&mut [u8])`/`write(&[u8])`/`write_all`/`flush`/`metadata`/`size` | ✅ 已完成 | [`n1c-rw-metadata.md`](../tasks/leaf/n1c-rw-metadata.md) |
| N2A | **stdout/stderr 模块**：`stdout`/`stderr`（`write`/`writeln`/`flush`）+ 绑定层 | ✅ 已完成 | [`n2a-stdout-stderr.md`](../tasks/leaf/n2a-stdout-stderr.md) |
| N2B | **stdin 增强**：`read_to_string`/`lines`（迭代行读取，复用 J1 循环形态） | ✅ 已完成 | [`n2b-stdin.md`](../tasks/leaf/n2b-stdin.md) |
| N3A | **`Path` 对象**：`Path::new`/`join`/`parent`/`file_name`/`path_extension`/`exists`/`is_file`/`is_dir` | ✅ 已完成 | [`n3a-path.md`](../tasks/leaf/n3a-path.md) |
| N3B | **`fs` 核心读写**：`fs::read_to_string`/`fs::write`/`fs::copy` | ✅ 已完成 | [`n3b-fs-rw.md`](../tasks/leaf/n3b-fs-rw.md) |
| N3C | **`fs` 目录操作**：`fs::remove_file`/`remove_dir_all`/`rename`/`create_dir`/`create_dir_all`/`read_dir`（目录条目迭代） | ✅ 已完成 | [`n3c-fs-dir.md`](../tasks/leaf/n3c-fs-dir.md) |
| N4 | **`eprintln!`/`eprint!` 宏**：内置格式化宏扩展 stderr 输出（typecheck 新增分支，desugar 复用格式化引擎 + 内建 stderr 打印，与 `println!` 同构） | ✅ 已完成 | [`n4-eprintln.md`](../tasks/leaf/n4-eprintln.md) |

**验收**：见任务树 [`stage-m-t.md`](../tasks/stage-m-t.md) 各子任务叶子的「验证」字段；全量回归通过。
