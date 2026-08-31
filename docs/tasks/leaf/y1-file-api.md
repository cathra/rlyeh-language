# Y1 File 目标 API

> **所属阶段**：阶段 Y
> **状态**：✅ 完成（降级：切片实参 `read(&mut [u8])`/`write(&[u8])` 受语言限制，以 `read(cap: i64) -> Result<String>` / `write(String) -> Result<i64>` 字节缓冲 API 提供等价能力；`open_with`/`Metadata` 已落地，2026-08-30）
> **依赖**：U1
> **后续（2026-08-30，S3）**：下列"切片实参不受支持"的语言限制已由切片类型系统（S1 类型层 / S2 codegen）解除。S3 新增切片形态 API `File::read_slice(&mut [u8])` / `write_slice(&[u8])`，**二进制安全**；因 Rlyeh 方法不支持重载，原名 `read(cap)` / `write(String)` 保留、切片版加 `_slice` 后缀。详见 [`slice-s3-std-wiring.md`](../leaf/slice-s3-std-wiring.md)。
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`open_with`/`read(&mut [u8])`/`write(&[u8])`/完整 `Metadata`。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`io/file.rl`：`open(path)` 默认只读兼容壳 ≡ `open_with(path, Read)`；`struct Metadata{size, mtime, is_file, is_dir}` + `File::metadata() -> Result<Metadata, IoError>`（4 槽非按值 calloc 堆对象）；driver 注入 `__rlyeh_file_size/mtime/mode` 平台内建（POSIX stat(2) 直读——Linux/macOS 偏移经本机 clang offsetof 实测、其余平台 -1 stub）；S_IFMT 掩码判定 is_file/is_dir。**`read(&mut [u8])`/`write(&[u8])` 切片实参挂 U1**（MVP 保留 `read(cap)`/`write(String)` 降级）。

## 风险评估（2026-08-28）

剩余 `read(&mut [u8])`/`write(&[u8])` 切片实参依赖 **U1 切片成熟**（`&mut [u8]`/`&[u8]` 切片类型作函数参数 + 切片值传递）。风险中——若 U1 切片参数化已完成则直接落地，否则需语言级增强。建议后续以独立子任务验证切片参数可行性后落地。

## 语言限制确认（2026-08-30）

实证：`&[u8]`/`&mut [u8]` **不可作为函数参数**——编译器报错 `unsupported syntax: 数组类型缺少大小 [T; N]`（Rlyeh AST 无无大小切片类型 `[T]`，切片类型仅支持定长 `[T; N]`）。故 `read(&mut [u8])`/`write(&[u8])` 字面签名无法落地。
替代方案评估：`Vec<u8>` 字节缓冲 IO 需 `fread`/`fwrite` 接收 `Vec<u8>`，而 extern 名即 C 符号、且 codegen 仅对 `LirType::Str`（String）在 extern 调用点特判取 data 指针（`Vec<u8>` 在 LIR 归为 `LirType::Ptr`，无独立变体，无法干净特判），故不经 codegen 改动无法以 `Vec<u8>` 作字节缓冲。
**结论**：Y1 以 MVP 字节缓冲 API 收口——`read(cap: i64) -> Result<String>` / `write(String) -> Result<i64>`（底层 libc `fread`/`fwrite` 经 String data 指针读写原始字节，二进制安全）；`open_with`/`Metadata`（size/mtime/is_file/is_dir）已完整落地。切片参数化形式与 `Vec<u8>` 字节缓冲留待语言层切片类型 / extern Vec 特判增强后补入，记为已知语言限制（非 Y1 阻塞项）。

## 验证

`file_open_with.{rlyeh,out}` + `io_file_test.rs` 新增 file_open_with_modes/file_metadata_complete（11/11 全绿）；`file_io.rl` 验证 `read(cap)`/`write(String)`/`read_to_string`/`write_all`（run-pass 套件全绿）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 标注剩余切片实参挂 U1 依赖 + 风险评估 |
| 2026-08-30 | Y1 收口（降级）：实证 `&[u8]`/`&mut [u8]` 切片参数不受语言支持；以 MVP 字节缓冲 API（`read(cap)->Result<String>`/`write(String)->Result<i64>` 经 libc fread/fwrite + String data 指针）收口，`open_with`/`Metadata` 完整；切片参数化与 `Vec<u8>` 字节缓冲记为已知语言限制 |
| 2026-08-30 | **限制解除 + 切片 API 落地（S3）**：切片类型系统（S1/S2）使 `&[T]`/`&mut [T]` 作参数可行；新增 `File::read_slice(&mut [u8])`/`write_slice(&[u8])`（经 driver 注入的 `__rlyeh_fread_ptr`/`__rlyeh_fwrite_ptr` 直传切片 data 指针），实测二进制安全（含 NUL 字节）。**`Vec<u8>` 仍非紧凑字节缓冲的等价替代**——实测 `[u8; N]` 数组非紧凑（8 字节/元素）而 `Vec<u8>` 紧凑（1 字节/元素），故字节切片须取自 `Vec<u8>`（`v.as_slice()` / `as_mut_slice()`），这印证本叶子当初选 `Vec<u8>`/String 缓冲的必要性。原 `read(cap)`/`write(String)` 保留 |
