# Y1 File 目标 API

> **所属阶段**：阶段 Y
> **状态**：🔧 部分完成
> **依赖**：U1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`open_with`/`read(&mut [u8])`/`write(&[u8])`/完整 `Metadata`。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`io/file.rl`：`open(path)` 默认只读兼容壳 ≡ `open_with(path, Read)`；`struct Metadata{size, mtime, is_file, is_dir}` + `File::metadata() -> Result<Metadata, IoError>`（4 槽非按值 calloc 堆对象）；driver 注入 `__rlyeh_file_size/mtime/mode` 平台内建（POSIX stat(2) 直读——Linux/macOS 偏移经本机 clang offsetof 实测、其余平台 -1 stub）；S_IFMT 掩码判定 is_file/is_dir。**`read(&mut [u8])`/`write(&[u8])` 切片实参挂 U1**（MVP 保留 `read(cap)`/`write(String)` 降级）。

## 验证

`file_open_with.{rlyeh,out}` + `io_file_test.rs` 新增 file_open_with_modes/file_metadata_complete（11/11 全绿）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
