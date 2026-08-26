# B2 io 模块（文件 IO）

> **所属阶段**：阶段 B
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

libc stdio 文件 IO 绑定。

## 背景

阶段 阶段 B 子任务，详见 阶段详情文档 [`stages/B.md`](../../stages/B.md)。

## 技术细节

`fopen/fread/fwrite/fclose/fseek/ftell` + POSIX `read`（fd 0 = stdin）；`c_str` NUL 结尾拷贝 + `read_file`/`write_file`/`append_file`/`read_line`；选 stdio 而非 open() 避平台 flags；编译器 extern `String` 参数 LIR 登记 `Str` + codegen `emit_call` bitcast 结构体指针→data 指针。

## 验证

`io_file_test.rs` 9 用例：往返/截断/追加/UTF-8/大文件/缺失/空/组合/hostname。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
