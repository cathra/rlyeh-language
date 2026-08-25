// ===== io 模块（B2，2026-08）：libc stdio 文件 IO + 控制台 IO =====
// 目录化（2026-08）：原 io.zeta 拆分 →
//   io/module.zeta   （模块根：OpenMode / open_mode_str / c_str + module 声明）
//   io/error.zeta （IoErrorKind / IoError / Error trait / kind_message）
//   io/file.zeta  （File 对象 + read_file/write_file/append_file）
//   io/console.zeta（Stdout/Stderr/stdout/stderr + stdin 读取/lines）
// 实现说明：
// - 基于通用 FFI（extern fn，阶段 A4）直接绑定 libc 符号，由链接器解析。
//   底层 extern（fopen/fread/fwrite/fclose/fseek/ftell/read）声明于根模块
//   core.zeta 的 extern 集中区；String 在 LIR 中即 data 指针（i8*），
//   可直接作为 C 字符串 / 缓冲传入 extern。
// - 路径参数需 NUL 结尾（c_str 构造）；读入缓冲后由调用方手动设置 len。

// N1a（2026-08）：文件打开模式（std-lib.md §4.1 OpenMode）。
// 与 File 对象（N1b）同域；core.zeta re-export 供用户裸名使用。
// 保持 io::OpenMode 完整路径（用户测试以 `io::OpenMode` 引用）。
enum OpenMode {
    Read,
    Write,
    Append,
    ReadWrite,
    Create,
}

// N1a：OpenMode → fopen 模式字符串映射（libc stdio，跨平台无 O_* flags 差异，
// 对比裸 POSIX open 的 O_CREAT/O_TRUNC 平台常量）。语义近似 OpenOptions：
// Read→"r" / Write→"w"（截断创建）/ Append→"a" / ReadWrite→"r+"（文件须存在）/
// Create→"w"（创建或截断）。
fn open_mode_str(mode: io::OpenMode) -> String {
    match mode {
        io::OpenMode::Read => String::from("r"),
        io::OpenMode::Write => String::from("w"),
        io::OpenMode::Append => String::from("a"),
        io::OpenMode::ReadWrite => String::from("r+"),
        io::OpenMode::Create => String::from("w"),
    }
}

// 复制 s 并附加 NUL 终止符，返回仅用于 C 字符串参数的拷贝。
// 返回值的 len 为内容长度（不含 NUL）；data[len] 处为 0。
fn c_str(s: String) -> String {
    let mut buf = String::with_capacity(s.len + 1);
    let mut i = 0;
    while i < s.len {
        buf.push_byte(s.data[i]);
        i = i + 1;
    }
    buf.push_byte(0);   // 附加 NUL（容量 s.len + 1 保证可写）
    buf.len = s.len;    // 恢复内容长度
    buf
}

// module 声明顺序即收集期注册顺序：error（IoError 基底）→ sendfile
// （file.zeta 的 File::sendfile_to 引用 io::sendfile::sendfile）→
// file → nio（依赖 error）→ console。
module error;
module sendfile;
module file;
module nio;
module console;
