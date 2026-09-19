// fs/module.rl：文件系统模块根——只做「子模块声明 + 暴露内容导出」（2026-09-19 重整）。
//
// 组成（拆分前自由函数内联于本文件，现下沉 fs/ops.rl）：
//   fs/path.rl（Path）
//   fs/ops.rl （read_to_string/write/copy/remove_file/rename/create_dir/
//               create_dir_all/remove_dir_all/read_dir）
// 实现说明（N3a/N3b/N3c）：
// - driver 不链接 rlyeh-std crate，无 Rust 绑定层，直接 libc extern（N1a 修订）。
// - `Path` 为纯字符串封装（无规范化/解析）；exists 用 POSIX access(F_OK)；
//   is_file 用 fopen 试探（无读权限文件误报，MVP）；is_dir = exists && !is_file 近似。
// - fs::read_to_string/fs::write 为 io::read_file/write_file 包装；copy = read + write。
// - `r#rename` 为根命名空间显式引用（见 fs/ops.rl）。
module path;
module ops;

// 自由函数重导出（保持 `fs::read_to_string` 等原全名可见）。
pub import ops::read_to_string;
pub import ops::write;
pub import ops::copy;
pub import ops::remove_file;
pub import ops::rename;
pub import ops::create_dir;
pub import ops::create_dir_all;
pub import ops::remove_dir_all;
pub import ops::read_dir;
