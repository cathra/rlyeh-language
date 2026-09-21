// io/module.rl：IO 模块根——只做「子模块声明 + 暴露内容导出」（2026-09-19 重整）。
//
// 组成（拆分前 OpenMode / c_str 内联于本文件，现下沉 io/base.rl）：
//   io/base.rl    （OpenMode / open_mode_str / c_str）
//   io/error.rl   （IoErrorKind / IoError / Error protocol / kind_message）
//   io/file.rl    （File 对象 + read_file/write_file/append_file）
//   io/console.rl （Stdout/Stderr/stdout/stderr + stdin 读取/lines）
//   io/nio.rl     （Interest/Event/Poller/set_nonblocking/is_nonblocking）
//   io/sendfile.rl（sendfile）
// 实现说明：
// - 基于通用 FFI（extern fn，阶段 A4）直接绑定 libc 符号，由链接器解析。
//   底层 extern（fopen/fread/fwrite/fclose/fseek/ftell/read）声明于 externs 单元；
//   String 在 LIR 中即 data 指针（i8*），可直接作为 C 字符串 / 缓冲传入 extern。
// - 路径参数需 NUL 结尾（c_str 构造）；读入缓冲后由调用方手动设置 len。
//
// module 声明顺序即收集期注册顺序：base（OpenMode 供 file 引用）→ error（IoError
// 基底）→ sendfile（file.rl 的 File::sendfile_to 引用 io::sendfile::sendfile）→
// file → nio（依赖 error）→ console。
module base;
module error;
module sendfile;
module file;
module nio;
module console;

// 重导出（保持 `io::OpenMode` / `io::c_str` 等原全名可见）。
pub import base::OpenMode;
pub import base::open_mode_str;
pub import base::c_str;
// EH-4（2026-09-21）：拥有型动态错误抽象 `DynError` 的别名定义在根单元
// （`rlyeh/module.rl`）——子模块文件中的 `type` 声明不参与收集。
