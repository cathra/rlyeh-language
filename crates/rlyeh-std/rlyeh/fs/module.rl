// fs 模块（std-lib.md §4.3 Path / §4.4 fs）。
// 目录化（2026-08）：原 fs.rl 拆分 → fs/module.rl（自由函数 + module 声明）+ fs/path.rl（Path）。
// 实现说明（N3a/N3b/N3c）：
// - driver 不链接 rlyeh-std crate，无 Rust 绑定层，直接 libc extern（N1a 修订）。
// - `Path` 为纯字符串封装（无规范化/解析）；exists 用 POSIX access(F_OK)；
//   is_file 用 fopen 试探（无读权限文件误报，MVP）；is_dir = exists && !is_file 近似。
// - fs::read_to_string/fs::write 为 io::read_file/write_file 包装；copy = read + write。
// - `r#rename` 为根命名空间显式引用：根 extern 与 fs::rename 同名，裸名 `rename(...)`
//   会被模块内优先解析绑定到 fs::rename（递归，返回 Result 与 0 比较触发
//   ChainTypeMismatch）；`r#` 前缀使 typecheck 跳过模块内优先，直接绑定根 extern（N3c）。

module path;

// N3b：读取整个文件（io::read_file 包装）。
fn read_to_string(path: String) -> Result<String, io::error::IoError> {
    read_file(path)
}

// N3b：写入整个文件（io::write_file 包装）。
fn write(path: String, content: String) -> Result<i64, io::error::IoError> {
    write_file(path, content)
}

// N3b：复制文件（read + write）。
fn copy(src: String, dst: String) -> Result<i64, io::error::IoError> {
    let content = read_file(src)?;
    write_file(dst, content)
}

// N3c：删除文件（POSIX unlink）。
fn remove_file(path: String) -> Result<i64, io::error::IoError> {
    let r = unlink(c_str(path));
    if r != 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::NotFound,
            String::from("remove_file failed"),
        ));
    }
    Result::Ok(1)
}

// N3c：重命名/移动（POSIX rename）。
fn rename(from: String, to: String) -> Result<i64, io::error::IoError> {
    let r = r#rename(c_str(from), c_str(to));
    if r != 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("rename failed"),
        ));
    }
    Result::Ok(1)
}

// N3c：创建目录（mode 0755）。
fn create_dir(path: String) -> Result<i64, io::error::IoError> {
    let r = mkdir(c_str(path), 493);   // 0755
    if r != 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::PermissionDenied,
            String::from("create_dir failed"),
        ));
    }
    Result::Ok(1)
}

// N3c：递归创建目录（逐级 mkdir：父路径经 Path::parent 递归向上）。
fn create_dir_all(path: String) -> Result<i64, io::error::IoError> {
    let exists = Path::new(String::from(path)).exists();
    if exists == 1 {
        return Result::Ok(1);
    }
    let parent = Path::new(String::from(path)).parent();
    let ps = parent.as_string();
    if ps.len > 0 {
        // 递归父路径（"." 时 exists 兜底返回 Ok(1)，无无限递归）
        match fs::create_dir_all(ps) {
            Ok(_) => {},
            Err(e) => return Result::Err(e),
        }
    }
    let r = mkdir(c_str(path), 493);
    if r != 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::PermissionDenied,
            String::from("create_dir_all failed"),
        ));
    }
    Result::Ok(1)
}

// N3c：递归删除目录（MVP：`rm -rf <path>`；危险操作——路径不含空格/通配符，MVP 工具性函数）。
fn remove_dir_all(path: String) -> Result<i64, io::error::IoError> {
    let cmd = String::from("rm -rf ") + path;
    let f = popen(c_str(cmd), String::from("r"));
    if f == 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("remove_dir_all failed"),
        ));
    }
    let mut tmp = String::with_capacity(16);
    let _n = fread(tmp, 1, 16, f);
    let _ = pclose(f);
    Result::Ok(1)
}

// N3c：列出目录条目（MVP：`ls -1 <path>` 捕获，`\n` 分隔；
// 依赖 /bin/sh；路径与文件名不含空格/换行；WASI/Windows 不支持）。
fn read_dir(path: String) -> Result<String, io::error::IoError> {
    let cmd = String::from("ls -1 ") + path;
    let f = popen(c_str(cmd), String::from("r"));
    if f == 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::NotFound,
            String::from("read_dir failed"),
        ));
    }
    let mut buf = String::new();
    loop {
        let mut tmp = String::with_capacity(256);
        let n = fread(tmp, 1, 256, f);
        if n == 0 {
            break;   // EOF
        }
        let mut i = 0;
        while i < n {
            buf.push_byte(tmp.data[i]);
            i = i + 1;
        }
    }
    let _ = pclose(f);
    Result::Ok(buf)
}
