// externs/module.rl：libc FFI extern 声明（2026-09-18 由 core.rl 拆分）。
// 注：extern 符号名须与 libc 一致，故本文件为平铺分片（不可模块化）。

// ===== extern 声明集中区（模块化拆分，2026-08-22）=====
// 编译器对 extern 按声明符号名生成 LLVM `declare`（`__rlyeh_` 前缀者由 driver
// 注入 define，跳过 declare）。符号名必须与 libc 一致，故不能放入子模块
// （模块前缀会改变 LLVM 符号名，链接器将无法解析 libc 符号）。
// 使用方见各子模块文件头注释（time.rl / io.rl / net.rl / sync.rl）。

// --- time.rl：进程 CPU 时钟 + 墙钟 ---
extern fn clock() -> i64;
// S2b：墙钟（clock_gettime CLOCK_MONOTONIC，返回微秒）。driver 注入 define
// （time_builtin_ir，见 rlyeh-driver lib.rs）；不支持的平台返回 -1，
// 语言侧 `Instant::now/elapsed` 退回 clock()（CPU 时钟）。
extern fn __rlyeh_clock_monotonic() -> i64;
// X1：系统时间（clock_gettime CLOCK_REALTIME，返回微秒）。driver 注入 define
// （time_builtin_ir，见 rlyeh-driver lib.rs）；不支持的平台返回 -1，
// 语言侧 `SystemTime::now` 退回 UNIX 纪元。
extern fn __rlyeh_clock_realtime() -> i64;

// --- io.rl：libc stdio + POSIX read ---
extern fn fopen(path: String, mode: String) -> i64;
extern fn fread(buf: String, size: i64, nmemb: i64, f: i64) -> i64;
extern fn fwrite(buf: String, size: i64, nmemb: i64, f: i64) -> i64;
// S3（2026-08-30）：切片 IO 转发——首参为裸指针，供 `&[u8]` / `&mut [u8]` 缓冲
// 直传 data 指针（上者 `String` 版经 codegen 特判取 data 指针，无法接收裸指针）。
// driver 注入 define 转发到 libc fread/fwrite（见 rlyeh-driver platform_ir.rs）。
extern fn __rlyeh_fread_ptr(ptr: *mut u8, size: i64, nmemb: i64, f: i64) -> i64;
extern fn __rlyeh_fwrite_ptr(ptr: *const u8, size: i64, nmemb: i64, f: i64) -> i64;
extern fn fclose(f: i64) -> i64;
extern fn fseek(f: i64, offset: i64, whence: i64) -> i64;
extern fn ftell(f: i64) -> i64;
extern fn fflush(f: i64) -> i32;   // N1c：File::flush（stdio 缓冲刷盘）
extern fn read(fd: i64, buf: String, count: i64) -> i64;   // 控制台读取（fd 0 = stdin）
// Y1（2026-08）：文件元数据（stat 平台差异由 driver 注入 define，见
// rlyeh-driver lib.rs file_stat_builtin_ir）：统一签名 (path) → 字段值；
// 失败（路径不存在等）返回 -1；非 Linux/macOS 平台注入 -1 stub。
// st_mode & S_IFMT 掩码：S_IFREG = 0x8000（普通文件），S_IFDIR = 0x4000。
extern fn __rlyeh_file_size(path: String) -> i64;
extern fn __rlyeh_file_mtime(path: String) -> i64;
extern fn __rlyeh_file_mode(path: String) -> i64;

// --- io/nio.rl + io/sendfile.rl：非阻塞 IO + 零拷贝传输（R 阶段，2026-08）---
extern fn fcntl(fd: i64, cmd: i64, arg: i64) -> i32;   // F_GETFL=3 / F_SETFL=4（O_NONBLOCK=0x4）；i32 返回 → extern_ret32 清洗
extern fn poll(fds: String, nfds: i64, timeout: i64) -> i32;   // pollfd 缓冲（8 字节/项）
// Y2a（2026-08-28）：kqueue/kevent（macOS/BSD 高性能事件后端）。kevent 结构体
// 32 字节缓冲（ident uintptr 8B + filter int16 2B + flags uint16 2B + fflags
// uint32 4B + data intptr 8B + udata ptr 8B），经 String 承载传 data 指针；
// timeout 为 `{timespec.tv_sec i64, tv_nsec i64}` 16 字节缓冲（String）或空。
extern fn __rlyeh_kqueue() -> i32;
extern fn __rlyeh_kevent(kq: i64, changelist: String, nchanges: i64, eventlist: String, nevents: i64, timeout: String) -> i32;
extern fn fileno(f: i64) -> i32;   // FILE* → 底层 fd（File::sendfile_to）
// sendfile(2) 平台差异由 driver 注入 define（见 rlyeh-driver lib.rs platform_builtin_ir）：
// 统一签名 (out_fd, in_fd, off_ptr, count)；WASI 下注入返回 -1 的 stub。
extern fn __rlyeh_sendfile(out_fd: i64, in_fd: i64, offset: String, count: i64) -> i64;

// --- thread（S 阶段，2026-08）：线程支持 ---
// pthread 差异由 driver 注入 define（thread_builtin_ir）：
// __rlyeh_thread_spawn(entry, arg) → 线程 tid 或 -1（pthread_create 封装）；
// entry 为函数指针值（语言侧经 extern i64 形参传地址整数，codegen ptrtoint）；
// __rlyeh_thread_join(tid) → 线程返回值槽内容或 -1；
// __rlyeh_thread_self() → 当前线程 id。
// __rlyeh_thread_sleep(micros) → 0 成功 / -1 失败（S2a，usleep 绑定）。
extern fn __rlyeh_thread_spawn(entry: i64, arg: i64) -> i64;
extern fn __rlyeh_thread_join(tid: i64) -> i64;
extern fn __rlyeh_thread_self() -> i64;
extern fn __rlyeh_thread_sleep(micros: i64) -> i64;
extern fn __rlyeh_thread_spawn_stack(entry: i64, arg: i64, stack_size: i64) -> i64;  // Y8：Builder::stack_size 定制线程栈（<=0 → 默认栈）

// --- 原子操作（H-M2 / SH-P0-4，2026-09-02）：`AtomicI64` 底层原语 ---
// 由 driver 注入 `define internal`（见 rlyeh-driver platform_ir.rs atomic_builtin_ir）：
// LLVM IR 级 `load atomic` / `store atomic` / `atomicrmw` / `cmpxchg` 实现，
// 无对应 C 链接符号（C11 `<stdatomic.h>` 的 atomic_fetch_add 为泛型宏，不可链接）。
// `__rlyeh_` 前缀：codegen 跳过 declare，避免与注入定义冲突。
// 指针参数 `p` 为 i64 句柄（8 字节对齐缓冲，std 侧经 calloc(1, 8) 分配）。
// RMW 族（swap / fetch_add / fetch_sub / fetch_and / fetch_or / fetch_xor）
// 与 cas 固定 SeqCst；load / store 另提供 acquire / release / relaxed 变体
// 供 `load_with` / `store_with` 按 Ordering 分派。
extern fn __rlyeh_atomic_load_i64_seq_cst(p: i64) -> i64;
extern fn __rlyeh_atomic_load_i64_acquire(p: i64) -> i64;
extern fn __rlyeh_atomic_load_i64_relaxed(p: i64) -> i64;
extern fn __rlyeh_atomic_store_i64_seq_cst(p: i64, v: i64) -> ();
extern fn __rlyeh_atomic_store_i64_release(p: i64, v: i64) -> ();
extern fn __rlyeh_atomic_store_i64_relaxed(p: i64, v: i64) -> ();
extern fn __rlyeh_atomic_swap_i64(p: i64, v: i64) -> i64;
extern fn __rlyeh_atomic_fetch_add_i64(p: i64, v: i64) -> i64;
extern fn __rlyeh_atomic_fetch_sub_i64(p: i64, v: i64) -> i64;
extern fn __rlyeh_atomic_fetch_and_i64(p: i64, v: i64) -> i64;
extern fn __rlyeh_atomic_fetch_or_i64(p: i64, v: i64) -> i64;
extern fn __rlyeh_atomic_fetch_xor_i64(p: i64, v: i64) -> i64;
extern fn __rlyeh_atomic_cas_i64(p: i64, expected: i64, desired: i64) -> i64;   // 返回旧值

// --- fs.rl：路径 + 文件系统 ---
extern fn access(path: String, mode: i64) -> i32;   // N3a：F_OK=0 存在性
extern fn unlink(path: String) -> i32;              // N3c：remove_file
extern fn r#rename(from: String, to: String) -> i32; // N3c：重命名/移动
extern fn mkdir(path: String, mode: i64) -> i32;    // N3c：create_dir
extern fn rmdir(path: String) -> i32;               // N3c：remove_dir（空目录）
extern fn popen(command: String, mode: String) -> i64;  // N3c：read_dir / remove_dir_all（ls / rm）
extern fn pclose(f: i64) -> i32;                    // N3c：popen 句柄关闭
extern fn write(fd: i64, buf: String, count: i64) -> i64;  // 控制台写入（N2a：fd 1 stdout / 2 stderr）

// --- net.rl：套接字 ---
// AF_UNIX=1, SOCK_STREAM=1（macOS/Linux 一致）
extern fn socketpair(domain: i64, type_: i64, proto: i64, fds: String) -> i32;
extern fn socket(domain: i64, type_: i64, proto: i64) -> i32;
extern fn connect(fd: i64, addr: String, len: i64) -> i32;
extern fn close(fd: i64) -> i32;
// O 阶段（2026-08）：TCP 服务器/HTTP 所需——bind/listen/accept/setsockopt/
// getsockname/shutdown。addr/len 传 String（codegen 取 data 指针）：
//  - `bind`/`connect` 的 addr 为 sockaddr_in 缓冲（sockaddr_in4 构造），len 定值 16
//  - `accept`/`getsockname` 的 addr/len 为 out 缓冲（内核回填），len 初值须为
//    socklen_t 小端 16（int_buf4），addr 容量 ≥16
extern fn bind(fd: i64, addr: String, len: i64) -> i32;
extern fn listen(fd: i64, backlog: i64) -> i32;
extern fn accept(fd: i64, addr: String, len: String) -> i32;
extern fn getsockname(fd: i64, addr: String, len: String) -> i32;
extern fn setsockopt(fd: i64, level: i64, optname: i64, optval: String, optlen: i64) -> i32;
extern fn shutdown(fd: i64, how: i64) -> i32;
// `send`/`recv` 为 actor 保留字，用原始标识符 `r#` 绕开（lexer 解为 Ident）
extern fn r#send(fd: i64, buf: String, len: i64, flags: i64) -> i64;
extern fn r#recv(fd: i64, buf: String, len: i64, flags: i64) -> i64;
// Y7（2026-08）：UDP——sendto/recvfrom（ssize_t i64 承载；recvfrom 的
// addr/addrlen 为输出回填缓冲 String，codegen 取 data 指针，与 accept/getsockname 一致）
extern fn sendto(fd: i64, buf: String, len: i64, flags: i64, addr: String, addrlen: i64) -> i64;
extern fn recvfrom(fd: i64, buf: String, len: i64, flags: i64, addr: String, addrlen: String) -> i64;
// 编译器注入的平台内建：返回当前目标 OS 码（0=未知 1=linux 2=macos 3=windows 4=freebsd）。
// 由 rlyeh-driver 在汇编阶段注入 `define internal i32 @__rlyeh_target_os()`；
// codegen 对 `__rlyeh_` 前缀 extern 不生成 declare（避免同符号 declare+define 冲突）。
extern fn __rlyeh_target_os() -> i32;
extern fn gethostname(name: String, len: i64) -> i64;

// --- sync.rl：pthread 互斥锁 / 读写锁 ---
// 分配用 `calloc` 而非 `malloc`：内建 alloc_array/alloc_bytes 已按
// `i8* @malloc(i64)` 声明 malloc，再以 i64 返回声明会触发 LLVM
// "invalid redefinition of function 'malloc'"；calloc 符号无内建冲突。
extern fn calloc(n: i64, size: i64) -> i64;
extern fn pthread_mutex_init(m: i64, attr: i64) -> i32;
extern fn pthread_mutex_lock(m: i64) -> i32;
extern fn pthread_mutex_unlock(m: i64) -> i32;
extern fn pthread_mutex_trylock(m: i64) -> i32;
extern fn pthread_rwlock_init(r: i64, attr: i64) -> i32;
extern fn pthread_rwlock_rdlock(r: i64) -> i32;
extern fn pthread_rwlock_wrlock(r: i64) -> i32;
extern fn pthread_rwlock_unlock(r: i64) -> i32;
extern fn pthread_rwlock_tryrdlock(r: i64) -> i32;
extern fn pthread_rwlock_trywrlock(r: i64) -> i32;
// P 阶段（2026-08）：条件变量 / 屏障（pthread_cond_* / pthread_barrier_*）
extern fn pthread_cond_init(c: i64, attr: i64) -> i32;
extern fn pthread_cond_destroy(c: i64) -> i32;
extern fn pthread_cond_wait(c: i64, m: i64) -> i32;
extern fn pthread_cond_signal(c: i64) -> i32;
extern fn pthread_cond_broadcast(c: i64) -> i32;
extern fn pthread_barrier_init(b: i64, attr: i64, count: i64) -> i32;
extern fn pthread_barrier_wait(b: i64) -> i32;
