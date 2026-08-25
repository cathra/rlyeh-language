# 04 · 文件与 IO

> 规范：docs/std-lib.md §4（IO 模块）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `File`：`create` / `open` + `OpenMode`（读 / 写 / 追加等）+ `write_all` / `read_to_string` / `close`
- 路径与文件系统：路径拼接、元信息查询、目录遍历
- 标准 IO：`stdout` / `stderr` 输出、`stdin` 读入
- 错误类型：`IoError` / `IoErrorKind`

## 示例清单

| 文件 | 说明 |
|------|------|
| `file_io.zeta` | File 创建 / 写入 / 读取 round-trip |
| `open_mode.zeta` | OpenMode 打开模式（读 / 写 / 追加） |
| `fs_ops.zeta` | 文件系统操作（元信息、存在性等） |
| `path_ops.zeta` | 路径操作（拼接 / 组件提取） |
| `fs_dir.zeta` | 目录遍历 |
| `stdout_stderr.zeta` | stdout / stderr 输出（含 `eprintln!`） |
| `stdin_enhance.zeta` | stdin 读入（需要管道/交互输入，见下） |
| `io_error_type.zeta` | IoError / IoErrorKind 构造与匹配 |

## 运行

```bash
zeta run examples/std-demos/04-file-io/file_io.zeta
zeta run examples/std-demos/04-file-io/fs_ops.zeta
# stdin 用例需提供输入：
echo -e "hello\n42" | zeta run examples/std-demos/04-file-io/stdin_enhance.zeta
```
