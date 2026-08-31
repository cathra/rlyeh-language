// io/file.rl：File 对象 + 文件级自由函数（std-lib.md §4.1）。
// 目录化（2026-08）：由原 io.rl 拆分。符号完整路径 io::file::File 等；
// OpenMode / c_str 保持 io::OpenMode / io::c_str（io/module.rl 顶层）。

// N1b（2026-08）：File 对象（句柄封装，std-lib.md §4.1）。
// Rlyeh 无 Drop，MVP 显式 close() 语义；字段对用户可见（MVP 无可见性控制）。
struct File {
    handle: i64,
    path: String,
    mode: io::OpenMode,
}

// Y1（2026-08）：文件元数据对象（std-lib.md §4.1 目标 Metadata）。
// size/mtime 为 i64（u64 目标 API 按项目惯例 i64 化）；is_file/is_dir
// 为 bool（经 st_mode & S_IFMT 掩码判定）。4 槽（2×i64 + 2×bool）→
// 非按值，指针稳定（calloc 堆对象，Result<Metadata> 泛型枚举亦禁用按值）。
// 注：须在 impl File 之前定义（typecheck 单遍顺序检查，前向引用报
// `undefined type`）。
struct Metadata {
    size: i64,
    mtime: i64,
    is_file: bool,
    is_dir: bool,
}

impl Metadata {
    // 文件大小（字节）。
    fn size(self) -> i64 {
        self.size
    }
    // 最后修改时间（epoch 秒，i64 化）。
    fn mtime(self) -> i64 {
        self.mtime
    }
    // 是否为普通文件。
    fn is_file(self) -> bool {
        self.is_file
    }
    // 是否为目录。
    fn is_dir(self) -> bool {
        self.is_dir
    }
}

impl File {
    // Y1（2026-08）：按模式打开文件（"r"/"w"/"a"/"r+"，经 open_mode_str 映射）。
    // §4.1 目标签名 `open_with(path, mode)`——保留原双参语义。
    fn open_with(path: String, mode: io::OpenMode) -> Result<io::file::File, io::error::IoError> {
        let cpath = c_str(path);
        let ms = io::open_mode_str(mode);
        let f = fopen(cpath, ms);
        if f == 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::NotFound,
                String::from("failed to open file"),
            ));
        }
        Result::Ok(io::file::File { handle: f, path: path, mode: mode })
    }
    // Y1：打开文件（默认只读模式，§4.1 兼容壳 ≡ open_with(path, Read)）。
    fn open(path: String) -> Result<io::file::File, io::error::IoError> {
        let cpath = c_str(path);
        let f = fopen(cpath, String::from("r"));
        if f == 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::NotFound,
                String::from("failed to open file"),
            ));
        }
        Result::Ok(io::file::File { handle: f, path: path, mode: io::OpenMode::Read })
    }
    // 创建文件（"w" 截断创建，与 write_file 语义一致）。
    fn create(path: String) -> Result<io::file::File, io::error::IoError> {
        let cpath = c_str(path);
        let f = fopen(cpath, String::from("w"));
        if f == 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::PermissionDenied,
                String::from("failed to create file"),
            ));
        }
        Result::Ok(io::file::File { handle: f, path: path, mode: io::OpenMode::Create })
    }
    // 显式关闭句柄（Rlyeh 无 Drop）；返回 1 表示已关闭。
    fn close(self) -> i64 {
        let _ = fclose(self.handle);
        1
    }
    // 打开模式（供调用方判断句柄语义）。
    fn mode(self) -> io::OpenMode {
        self.mode
    }
    // N1c：读取剩余全部内容（从当前位置到 EOF）；失败返回 Err(IoError)。
    fn read_to_string(&mut self) -> Result<String, io::error::IoError> {
        let _end = fseek(self.handle, 0, 2);   // SEEK_END = 2，文件字节数
        let size = ftell(self.handle);
        let _set = fseek(self.handle, 0, 0);   // SEEK_SET = 0，回到开头
        let mut buf = String::with_capacity(size);
        let n = fread(buf, 1, size, self.handle);
        buf.len = n;                           // 实际读入字节数（空文件 n = 0）
        Result::Ok(buf)
    }
    // 读取至多 cap 字节；返回按实际字节数设 len 的内容
    // （MVP：String 缓冲实参，`&mut [u8]` 切片实参留待切片借用成熟）。
    fn read(&mut self, cap: i64) -> Result<String, io::error::IoError> {
        let _ = fseek(self.handle, 0, 1);   // SEEK_CUR：写→读切换定位（C 更新模式要求）
        let mut buf = String::with_capacity(cap);
        let n = fread(buf, 1, cap, self.handle);
        if n < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("read failed"),
            ));
        }
        buf.len = n;
        Result::Ok(buf)
    }
    // 写入 buf 全部内容；返回写入字节数
    // （MVP：String 入参，`&[u8]` 切片实参留待切片借用成熟）。
    fn write(&mut self, buf: String) -> Result<i64, io::error::IoError> {
        let _ = fseek(self.handle, 0, 1);   // SEEK_CUR：读→写切换定位（C 更新模式要求）
        let n = fwrite(buf, 1, buf.len, self.handle);
        Result::Ok(n)
    }
    // S3（2026-08-30）：切片形参的二进制安全读写——`&mut [u8]` / `&[u8]`。
    // 原名 `read` / `write` 已被 `cap: i64` / `String` 版占用（Rlyeh 方法不支持
    // 重载，同签名只能有一个），故切片版以 `read_slice` / `write_slice` 提供；
    // 底层经 `__rlyeh_fread_ptr` / `__rlyeh_fwrite_ptr` 直传切片 data 指针
    // （driver 注入 define 转发到 libc fread / fwrite）。
    fn read_slice(&mut self, buf: &mut [u8]) -> Result<i64, io::error::IoError> {
        let _ = fseek(self.handle, 0, 1);   // SEEK_CUR：写→读切换定位
        let n = __rlyeh_fread_ptr(buf.as_mut_ptr(), 1, buf.len(), self.handle);
        if n < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("read_slice failed"),
            ));
        }
        Result::Ok(n)
    }
    fn write_slice(&mut self, buf: &[u8]) -> Result<i64, io::error::IoError> {
        let _ = fseek(self.handle, 0, 1);   // SEEK_CUR：读→写切换定位
        let n = __rlyeh_fwrite_ptr(buf.as_ptr(), 1, buf.len(), self.handle);
        Result::Ok(n)
    }
    // 写满全部内容（MVP 单次 fwrite 即全量，语义等同 write）。
    fn write_all(&mut self, buf: String) -> Result<i64, io::error::IoError> {
        let _ = fseek(self.handle, 0, 1);   // SEEK_CUR：读→写切换定位
        let n = fwrite(buf, 1, buf.len, self.handle);
        Result::Ok(n)
    }
    // 刷新 stdio 缓冲到 OS；失败返回 Err(IoError)。
    fn flush(&mut self) -> Result<i64, io::error::IoError> {
        let r = fflush(self.handle);
        if r != 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("flush failed"),
            ));
        }
        Result::Ok(1)
    }
    // 文件大小（fseek SEEK_END + ftell，恢复原读取位置）。
    fn size(&mut self) -> Result<i64, io::error::IoError> {
        let cur = ftell(self.handle);
        let _end = fseek(self.handle, 0, 2);
        let size = ftell(self.handle);
        let _set = fseek(self.handle, 0, cur);
        Result::Ok(size)
    }
    // Y1：完整元数据（size/mtime/is_file/is_dir）。stat 经 driver 注入的
    // __rlyeh_file_mode/size/mtime 平台内建（Linux/macOS 原生 stat，其他
    // 平台 stub 返回 -1）；失败（路径不存在等）返回 Err(IoError)。
    fn metadata(&mut self) -> Result<io::file::Metadata, io::error::IoError> {
        let cpath = c_str(self.path);
        let mode = __rlyeh_file_mode(cpath);
        if mode < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::NotFound,
                String::from("stat failed"),
            ));
        }
        let size = __rlyeh_file_size(cpath);
        let mtime = __rlyeh_file_mtime(cpath);
        // POSIX S_IFMT = 0xF000；S_IFREG = 0x8000（普通文件），S_IFDIR = 0x4000。
        let is_file = (mode & 0xF000) == 0x8000;
        let is_dir = (mode & 0xF000) == 0x4000;
        Result::Ok(io::file::Metadata {
            size: size,
            mtime: mtime,
            is_file: is_file,
            is_dir: is_dir,
        })
    }
    // R3（2026-08）：零拷贝发送本文件到 socket（offset 起至 EOF）。
    // FILE* 经 fileno 取底层 fd 后走 sendfile(2)；返回实际发送字节数；
    // 失败返回 Err(IoError)。调用方须已 flush（stdio 缓冲落盘后 sendfile 才可见）。
    fn sendfile_to(&self, sock_fd: i64, offset: i64) -> Result<i64, io::error::IoError> {
        let fd = fileno(self.handle);
        if fd < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("fileno failed"),
            ));
        }
        io::sendfile::sendfile(sock_fd, fd, offset, 0)   // count = 0：发送到 EOF
    }
}

// M3a（2026-08）：io 自由函数 Result 化——返回 `Result<T, IoError>`，
// 失败不再用"空串 / -1"哨兵值。MVP 错误映射：打开失败统一按
// IoErrorKind 分类（read→NotFound / write/append→PermissionDenied），
// 不区分底层 errno（errno extern 读取留待后续）。
// 单次 fread：文件大小已知（fseek SEEK_END + ftell），普通文件一次可读满。
fn read_file(path: String) -> Result<String, io::error::IoError> {
    let cpath = c_str(path);
    let f = fopen(cpath, String::from("r"));
    if f == 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::NotFound,
            String::from("failed to open file for read"),
        ));
    }
    let _end = fseek(f, 0, 2);   // SEEK_END = 2，文件字节数
    let size = ftell(f);
    let _set = fseek(f, 0, 0);   // SEEK_SET = 0，回到开头
    let mut buf = String::with_capacity(size);
    let n = fread(buf, 1, size, f);
    buf.len = n;                 // 实际读入字节数（空文件 n = 0）
    let _ = fclose(f);
    Result::Ok(buf)
}

// 写入整个文件（"w" 截断写）；成功返回写入字节数，失败返回 Err(IoError)。
fn write_file(path: String, content: String) -> Result<i64, io::error::IoError> {
    let cpath = c_str(path);
    let f = fopen(cpath, String::from("w"));
    if f == 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::PermissionDenied,
            String::from("failed to open file for write"),
        ));
    }
    let n = fwrite(content, 1, content.len, f);
    let _ = fclose(f);
    Result::Ok(n)
}

// 追加写入（"a"）；成功返回写入字节数，失败返回 Err(IoError)。
fn append_file(path: String, content: String) -> Result<i64, io::error::IoError> {
    let cpath = c_str(path);
    let f = fopen(cpath, String::from("a"));
    if f == 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::PermissionDenied,
            String::from("failed to open file for append"),
        ));
    }
    let n = fwrite(content, 1, content.len, f);
    let _ = fclose(f);
    Result::Ok(n)
}
