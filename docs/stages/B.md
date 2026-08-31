# 阶段 B — 标准库完善

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-a-f.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| B1 | 时间模块：`Duration { micros: i64 }` + `Instant { start: i64 }`，底层 libc `clock()` extern（首例 extern 驱动标准库模块） | ✅ 完成 | [`b1-time-module.md`](../tasks/leaf/b1-time-module.md) |
| B2 | io 模块：libc stdio 文件 IO（`fopen`/`fread`/`fwrite`/`fclose`/`fseek`/`ftell`）+ `read_file`/`write_file`/`append_file`/`c_str`/`read_line`；编译器 extern `String` 参数取 data 指针 | ✅ 完成 | [`b2-io-module.md`](../tasks/leaf/b2-io-module.md) |
| B3 | net 模块：`hostname()` + `htons` + `socketpair_stream`/`fd_at`（AF_UNIX 全双工字节流 + int32 字节解释）+ `send_all`/`recv_some` + `sockaddr_in4`（macOS 布局字节打包）/`tcp_connect`（socket→sockaddr→connect，失败 close 返回 -1）；**依赖本轮位运算全链路**（`&`/` | `/`^`/`<<`/`>>`，含算术右移 ashr）；NIO/sendfile 绑定层接线随阶段 C/D 延后 | [`b3-net-module.md`](../tasks/leaf/b3-net-module.md) |
| B4 | sync 模块：`Mutex`/`RwLock`（pthread extern + calloc 承载，try 系列依赖 extern `i32` 返回支持）；Condvar/Barrier 骨架留注释（待函数指针/线程创建） | ✅ 完成 | [`b4-sync-module.md`](../tasks/leaf/b4-sync-module.md) |
| B5 | collections 补全：`Vec` 的 `first`/`last`/`reverse`/`swap`/`binary_search`；`HashMap` `len`/`is_empty` 固化与去重 | ✅ 完成 | [`b5-vec-hashmap-methods.md`](../tasks/leaf/b5-vec-hashmap-methods.md) |
