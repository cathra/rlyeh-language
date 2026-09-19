// io/base.rl：IO 基础类型与辅助（2026-09-19 由 io/module.rl 拆出）。
//
// 归属子模块 `io::base`，经 io/module.rl 的 `pub import` 重导出为
// `io::OpenMode` / `io::open_mode_str` / `io::c_str` 原全名。
// - 路径参数需 NUL 结尾（c_str 构造）；读入缓冲后由调用方手动设置 len。
// - String 在 LIR 中即 data 指针（i8*），可直接作为 C 字符串 / 缓冲传入 extern。

// N1a（2026-08）：文件打开模式（std-lib.md §4.1 OpenMode）。
// 与 File 对象（N1b）同域；core 重导出供用户裸名使用。
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
