# File / Path / fs（IO 模块）

文件与文件系统 API，位于 `io` 子模块（`import io::File;` 等已在根模块重导出）。

> **C 程序员对照**：Rlyeh 的 `File` ≈ C 的 `FILE*`（`fopen`/`fread`/`fwrite`/`fclose`）或 `int fd`（`open`/`read`/`write`/`close`）。最大区别：**离开作用域自动 `close`**（RAII），不会像 C 那样忘记 `fclose` 导致句柄泄漏。`seek` 的 `whence` 语义与 C 完全一致：`0`=头(`SEEK_SET`)、`1`=当前(`SEEK_CUR`)、`2`=尾(`SEEK_END`)。

## 便捷函数

### `io::read_file(path: String) -> String`
一次性读取整个文本文件。
```rlyeh
let s = io::read_file(String::from("a.txt"));
println(s);
```

### `io::write_file(path: String, content: String) -> ()`
写入整个文本文件（覆盖）。
```rlyeh
io::write_file(String::from("a.txt"), String::from("hi"));
```

## File

### 构造

```rlyeh
let f = File::open(String::from("a.txt"));        // 只读打开
let f = File::create(String::from("b.txt"));      // 创建/截断
let f = File::append(String::from("c.txt"));      // 追加
let f = File::from_raw_fd(3);                     // 从 fd 接管
```

### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `write_all` | `(buf: &[u8]) -> ()` | 写入全部字节 |
| `flush` | `() -> ()` | 刷盘 |
| `read` | `(buf: &mut [u8]) -> i64` | 读入缓冲，返回字节数（EOF 为 0） |
| `read_line` | `(&mut String) -> i64` | 读一行到 `String`（含结尾换行） |
| `seek` | `(pos: i64, whence: i64) -> i64` | 定位（`whence`：0=头 1=当前 2=尾），返回新偏移 |
| `is_eof` | `() -> bool` | 是否到文件尾 |
| `file_size` | `() -> i64` | 文件字节大小 |
| `set_len` | `(size: i64) -> ()` | 截断/扩展文件长度 |
| `sync_all` | `() -> ()` | 同步到磁盘 |
| `metadata` | `() -> FileMetadata` | 文件元数据（size / is_dir / is_file / perms） |
| `close` | `() -> ()` | 关闭文件 |

```rlyeh
let f = File::create(String::from("log.txt"));
f.write_all("hello");
f.flush();
let m = f.metadata();
println(m.size);
```

### `fstat(fd: i64) -> FileMetadata`
从 fd 查元数据（自由函数）。
```rlyeh
let m = fstat(1);
```

## Path（自由函数）

| 函数 | 签名 | 说明 |
|------|------|------|
| `exists` | `(path: String) -> bool` | 路径是否存在 |
| `is_file` | `(path: String) -> bool` | 是否为文件 |
| `is_dir` | `(path: String) -> bool` | 是否为目录 |
| `is_symlink` | `(path: String) -> bool` | 是否为符号链接 |
| `dir_list` | `(path: String) -> Vec<String>` | 列出目录项 |
| `dir_walk` | `(path: String, cb: fn(String)) -> ()` | 递归遍历（回调为无捕获闭包） |

```rlyeh
println(Path::exists(String::from("a.txt")));
let items = Path::dir_list(String::from("."));
```

## fs 函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `fs_read` | `(path: String) -> Vec<u8>` | 读二进制为字节数组 |
| `fs_write` | `(path: String, buf: &[u8]) -> ()` | 写二进制 |
| `ls` | `(path: String) -> Vec<String>` | 列目录 |

## sendfile

```rlyeh
// sendfile(out_fd, in_fd, offset, count) -> 已传输字节数（零拷贝，规划中）
let n = sendfile(out_fd, in_fd, 0, 4096);
```

## 完整示例

```rlyeh
fn main() {
    io::write_file(String::from("demo.txt"), String::from("line1\nline2"));
    let content = io::read_file(String::from("demo.txt"));
    let f = File::open(String::from("demo.txt"));
    let mut buf: Vec<u8> = Vec::with_capacity(64);
    let n = f.read(buf.as_mut_slice());
    let meta = f.metadata();
    println(meta.size);
    println(Path::is_file(String::from("demo.txt")));
}
```

---

[← 返回标准库详述索引](./index.md)
