//! 平台内建 LLVM IR 定义（driver 按目标注入）。
//!
//! 由 lib.rs 拆分而来：、sendfile/thread/time/file_stat 平台实现。

use super::*;

/// 注入平台内建的 LLVM IR 定义文本（`__rlyeh_target_os` 返回当前目标 OS 码）。
pub(crate) fn platform_builtin_ir(target: Option<&str>) -> String {
    let os = target_os_code(target);
    format!(
        "\n; --- 平台内建（driver 按目标注入）---\ndefine internal i32 @__rlyeh_target_os() {{\nentry:\n  ret i32 {}\n}}\n{}\n{}\n{}\n{}\n",
        os,
        sendfile_builtin_ir(os),
        thread_builtin_ir(os),
        time_builtin_ir(os),
        file_stat_builtin_ir(os)
    )
}

/// `__rlyeh_sendfile(i64 out_fd, i64 in_fd, i64* off, i64 count) -> i64` 的平台实现。
/// 语言侧 io::sendfile::sendfile 统一调用此符号，平台签名差异在此屏蔽：
/// - Linux（码 1）：`ssize_t sendfile(int out, int in, off_t* off, size_t count)`，
///   off 为 in/out 指针（count==0 发送到 EOF，返回实际字节数，失败 -1）。
/// - macOS（码 2）：`int sendfile(int in, int out, off_t off, off_t* len, sf_hdtr*, int flags)`，
///   off 传值、len in/out（初值=count，0 到 EOF；成功返回 0，实际字节回填 len），
///   失败 -1。注意 macOS 更新的是 len 而非 off，调用方按返回值推进偏移。
/// - 其他平台（freebsd/windows/wasi 等）：返回 -1（Unsupported，MVP 禁用文档化）。
pub(crate) fn sendfile_builtin_ir(os: i32) -> String {
    match os {
        1 => r#"
; Linux：sendfile(2) 4 参；off 指针 in/out，count==0 到 EOF
declare i64 @sendfile(i64, i64, i64*, i64)
define internal i64 @__rlyeh_sendfile(i64 %out_fd, i64 %in_fd, i64* %off, i64 %count) {
entry:
  %r = call i64 @sendfile(i64 %out_fd, i64 %in_fd, i64* %off, i64 %count)
  ret i64 %r
}
"#
        .to_string(),
        2 => r#"
; macOS：sendfile(2) 6 参；off 传值、len in/out（初值=count），成功 0 / 失败 -1
declare i32 @sendfile(i64, i64, i64, i64*, i64, i32)
define internal i64 @__rlyeh_sendfile(i64 %out_fd, i64 %in_fd, i64* %off, i64 %count) {
entry:
  %offv = load i64, i64* %off
  %len = alloca i64
  store i64 %count, i64* %len
  %r = call i32 @sendfile(i64 %in_fd, i64 %out_fd, i64 %offv, i64* %len, i64 0, i32 0)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %done, label %fail
done:
  %n = load i64, i64* %len
  ret i64 %n
fail:
  ret i64 -1
}
"#
        .to_string(),
        _ => r#"
; 其他平台（freebsd/windows/wasi）：sendfile(2) 不可用，返回 -1
define internal i64 @__rlyeh_sendfile(i64 %out_fd, i64 %in_fd, i64* %off, i64 %count) {
entry:
  ret i64 -1
}
"#
        .to_string(),
    }
}

/// 线程平台内建（S0，2026-08）：`__rlyeh_thread_spawn/__rlyeh_thread_join/__rlyeh_thread_self`；
/// S2a（2026-08）增补 `__rlyeh_thread_sleep`（usleep 绑定）。
/// 语言侧 `thread::Thread::spawn/join/current` 与 `thread::sleep` 统一调用，pthread 差异在此屏蔽：
/// - Linux/macOS（码 1/2）：pthread_create/join/self/usleep 完整实现。线程入口为
///   `i64 (i8*)*`——Rlyeh `fn() -> i64` 函数指针值（按地址整数经语言侧 extern 传入，
///   参数位 i64）在此 `inttoptr` + `bitcast` 后交给 pthread_create；被调线程忽略
///   argv（i8* 多余参数，ABI 安全），返回值 i64 与 void* 同寄存器，join 经 i64* 槽读回。
///   sleep 经 `usleep(3)`（POSIX，微秒，useconds_t 截断 u32，上限约 71 分钟）。
/// - 其他平台（freebsd/windows/wasi）：返回 -1（Unsupported，MVP 禁用文档化）。
/// S0e（2026-08）：默认栈大小确认——pthread_create 传 `attr = null`，使用系统默认栈：
///   Linux（glibc，os=1）新线程默认栈 8MB（受 ulimit -s 约束，多数发行版 8MB）；
///   macOS（os=2）新线程默认栈约 512KB（POSIX 默认，PTHREAD_STACK_MIN 之上）。MVP 不
///   提供 `thread::Builder::stack_size` 等定制（规划）；需要大栈的深递归场景在 Linux
///   下经 `ulimit -s` 生效，macOS 下规划显式 pthread_attr_setstacksize 注入。
/// S0e 线程局部状态与内存模型：MVP 无 TLS / 线程局部状态需求（无 thread_local 关键字
///   与 __thread 段生成）；线程间共享数据经 `Rc<Channel>`（Mutex + Condvar 队列，P1）
///   或 `Arc` 等同步原语；每个线程独立栈 + 独立寄存器上下文，堆共享（Rc/Box 指针
///   跨线程传递须经同步原语保证可见性，MVP 无内存模型排序保证，数据竞争 UB 由调用方
///   负责——与 C 并发内存模型一致）。
pub(crate) fn thread_builtin_ir(os: i32) -> String {
    if os == 1 || os == 2 {
        r#"
; --- 线程平台内建（pthread）---
declare i32 @pthread_create(i64*, i64*, i64 (i8*)*, i8*)
declare i32 @pthread_join(i64, i64*)
declare i64 @pthread_self()
declare i32 @usleep(i32)
declare i32 @pthread_attr_init(i8*)
declare i32 @pthread_attr_destroy(i8*)
declare i32 @pthread_attr_setstacksize(i8*, i64)
define internal i64 @__rlyeh_thread_spawn(i64 %ep_addr, i64 %arg) {
entry:
  %tid = alloca i64
  %ep = inttoptr i64 %ep_addr to i8*
  %start = bitcast i8* %ep to i64 (i8*)*
  %argp = inttoptr i64 %arg to i8*
  %r = call i32 @pthread_create(i64* %tid, i64* null, i64 (i8*)* %start, i8* %argp)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %done, label %fail
done:
  %tv = load i64, i64* %tid
  ret i64 %tv
fail:
  ret i64 -1
}
; Y8：__rlyeh_thread_spawn_stack(entry, arg, stack_size)
; stack_size <= 0 → 系统默认栈（null attr，与 __rlyeh_thread_spawn 等价）；
; 否则 pthread_attr_setstacksize 定制线程栈（须 >= PTHREAD_STACK_MIN）。
define internal i64 @__rlyeh_thread_spawn_stack(i64 %ep_addr, i64 %arg, i64 %stack_size) {
entry:
  %tid0 = alloca i64
  %tid = alloca i64
  %attr = alloca i8, i64 128, align 16
  %usep = icmp sgt i64 %stack_size, 0
  br i1 %usep, label %withattr, label %noattr
noattr:
  %ep0 = inttoptr i64 %ep_addr to i8*
  %start0 = bitcast i8* %ep0 to i64 (i8*)*
  %argp0 = inttoptr i64 %arg to i8*
  %r0 = call i32 @pthread_create(i64* %tid0, i64* null, i64 (i8*)* %start0, i8* %argp0)
  %ok0 = icmp eq i32 %r0, 0
  br i1 %ok0, label %done0, label %fail0
done0:
  %tv0 = load i64, i64* %tid0
  ret i64 %tv0
fail0:
  ret i64 -1
withattr:
  %ai = call i32 @pthread_attr_init(i8* %attr)
  %aiok = icmp eq i32 %ai, 0
  br i1 %aiok, label %setss, label %fail_ai
setss:
  %ss = call i32 @pthread_attr_setstacksize(i8* %attr, i64 %stack_size)
  %ssok = icmp eq i32 %ss, 0
  br i1 %ssok, label %create, label %fail_ss
create:
  %ep = inttoptr i64 %ep_addr to i8*
  %start = bitcast i8* %ep to i64 (i8*)*
  %argp = inttoptr i64 %arg to i8*
  %attrp = bitcast i8* %attr to i64*
  %r = call i32 @pthread_create(i64* %tid, i64* %attrp, i64 (i8*)* %start, i8* %argp)
  call i32 @pthread_attr_destroy(i8* %attr)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %done, label %fail
done:
  %tv = load i64, i64* %tid
  ret i64 %tv
fail:
  ret i64 -1
fail_ss:
  call i32 @pthread_attr_destroy(i8* %attr)
  ret i64 -1
fail_ai:
  ret i64 -1
}
define internal i64 @__rlyeh_thread_join(i64 %tid) {
entry:
  %rv = alloca i64
  store i64 0, i64* %rv
  %r = call i32 @pthread_join(i64 %tid, i64* %rv)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %done, label %fail
done:
  %v = load i64, i64* %rv
  ret i64 %v
fail:
  ret i64 -1
}
define internal i64 @__rlyeh_thread_self() {
entry:
  %r = call i64 @pthread_self()
  ret i64 %r
}
define internal i64 @__rlyeh_thread_sleep(i64 %micros) {
entry:
  %us = trunc i64 %micros to i32
  %r = call i32 @usleep(i32 %us)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %done, label %fail
done:
  ret i64 0
fail:
  ret i64 -1
}
"#
        .to_string()
    } else {
        r#"
; --- 线程平台内建（其他平台禁用）---
define internal i64 @__rlyeh_thread_spawn(i64 %ep_addr, i64 %arg) {
entry:
  ret i64 -1
}
define internal i64 @__rlyeh_thread_spawn_stack(i64 %ep_addr, i64 %arg, i64 %stack_size) {
entry:
  ret i64 -1
}
define internal i64 @__rlyeh_thread_join(i64 %tid) {
entry:
  ret i64 -1
}
define internal i64 @__rlyeh_thread_self() {
entry:
  ret i64 -1
}
define internal i64 @__rlyeh_thread_sleep(i64 %micros) {
entry:
  ret i64 -1
}
"#
        .to_string()
    }
}

/// 时间平台内建（墙钟，S2b）：
/// - `__rlyeh_clock_monotonic() -> i64`：`clock_gettime(CLOCK_MONOTONIC)` 微秒值。
///   Linux（os 1）/macOS（os 2）为真实现，timespec 经 [2 x i64] 缓冲传指针，
///   `tv_sec*1e6 + tv_nsec/1000`；失败返回 -1。
///   注意：CLOCK_MONOTONIC 常量随平台不同——Linux = 1，Darwin(macOS) = 6；
///   CLOCK_REALTIME 在 Linux / Darwin 均为 0（X1，SystemTime 用）。
/// - 其他平台（freebsd/windows/wasi）：两个内建均返回 -1（Unsupported，
///   语言侧 `Instant::now/elapsed` 退回 `clock()` CPU 时钟、`SystemTime::now`
///   退回 UNIX 纪元，保持可用）。
pub(crate) fn time_builtin_ir(os: i32) -> String {
    if os == 1 || os == 2 {
        let monotonic = if os == 2 { 6 } else { 1 };
        let realtime = 0;
        format!(
            r#"
; --- 时间平台内建（墙钟：clock_gettime CLOCK_MONOTONIC={monotonic} / CLOCK_REALTIME={realtime}）---
declare i32 @clock_gettime(i32, i64*)
define internal i64 @__rlyeh_clock_now(i32 %clk_id) {{
entry:
  %ts = alloca [2 x i64]
  %tsb = bitcast [2 x i64]* %ts to i8*
  %tsg0 = getelementptr i8, i8* %tsb, i64 0
  %p = bitcast i8* %tsg0 to i64*
  %r = call i32 @clock_gettime(i32 %clk_id, i64* %p)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %done, label %fail
done:
  %sec = load i64, i64* %p
  %sec_us = mul i64 %sec, 1000000
  %tsg1 = getelementptr i8, i8* %tsb, i64 8
  %nsptr = bitcast i8* %tsg1 to i64*
  %ns = load i64, i64* %nsptr
  %ns_us = udiv i64 %ns, 1000
  %total = add i64 %sec_us, %ns_us
  ret i64 %total
fail:
  ret i64 -1
}}
define internal i64 @__rlyeh_clock_monotonic() {{
entry:
  %r = call i64 @__rlyeh_clock_now(i32 {monotonic})
  ret i64 %r
}}
define internal i64 @__rlyeh_clock_realtime() {{
entry:
  %r = call i64 @__rlyeh_clock_now(i32 {realtime})
  ret i64 %r
}}
"#
        )
        .to_string()
    } else {
        r#"
; --- 时间平台内建（其他平台禁用，退回 clock()/UNIX 纪元）---
define internal i64 @__rlyeh_clock_monotonic() {
entry:
  ret i64 -1
}
define internal i64 @__rlyeh_clock_realtime() {
entry:
  ret i64 -1
}
"#
        .to_string()
    }
}

/// Y1（2026-08）：文件元数据平台内建（`File::metadata` 的 size/mtime/mode）。
///
/// 统一语言侧签名 `__rlyeh_file_size/mtime/mode(path: String) -> i64`——
/// extern String 实参经 codegen 自动取 data 指针（与 `fopen` 同款），故
/// 此处入参为 `i8*`（NUL 结尾 C 路径）。三个入口各自 `stat(2)` 一次并
/// 读取对应 `struct stat` 字段；失败（路径不存在等）返回 -1。
///
/// 字段偏移为平台 ABI（`<sys/stat.h>` 布局，macOS 偏移经本机 clang
/// `offsetof` 实测：st_mode@4 / st_size@96 / st_mtimespec.tv_sec@48）：
/// - Linux x86_64：st_mode@24（mode_t u32）/ st_size@48（off_t i64）/ st_mtime@88（timespec.tv_sec）；
/// - macOS：st_mode@4（mode_t u16）/ st_size@96 / st_mtime@48（mtimespec.tv_sec）；
/// - 其余平台（Windows/WASI 等）：无 POSIX stat，注入返回 -1 的 stub。
pub(crate) fn file_stat_builtin_ir(os: i32) -> String {
    match os {
        1 => r#"
; --- Y1 文件元数据（Linux x86_64 struct stat：mode@24 / size@48 / mtime@88）---
declare i32 @stat(i8*, i8*)
define internal i64 @__rlyeh_file_size(i8* %path) {
entry:
  %st = alloca [160 x i8], align 8
  %r = call i32 @stat(i8* %path, i8* %st)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %okbb, label %err
err:
  ret i64 -1
okbb:
  %p = getelementptr i8, i8* %st, i64 48
  %pv = bitcast i8* %p to i64*
  %v = load i64, i64* %pv
  ret i64 %v
}
define internal i64 @__rlyeh_file_mtime(i8* %path) {
entry:
  %st = alloca [160 x i8], align 8
  %r = call i32 @stat(i8* %path, i8* %st)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %okbb, label %err
err:
  ret i64 -1
okbb:
  %p = getelementptr i8, i8* %st, i64 88
  %pv = bitcast i8* %p to i64*
  %v = load i64, i64* %pv
  ret i64 %v
}
define internal i64 @__rlyeh_file_mode(i8* %path) {
entry:
  %st = alloca [160 x i8], align 8
  %r = call i32 @stat(i8* %path, i8* %st)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %okbb, label %err
err:
  ret i64 -1
okbb:
  %p = getelementptr i8, i8* %st, i64 24
  %pv = bitcast i8* %p to i32*
  %m32 = load i32, i32* %pv
  %m = zext i32 %m32 to i64
  ret i64 %m
}
"#
        .to_string(),
        2 => r#"
; --- Y1 文件元数据（macOS struct stat：mode@4 u16 / size@96 / mtime@48）---
declare i32 @stat(i8*, i8*)
define internal i64 @__rlyeh_file_size(i8* %path) {
entry:
  %st = alloca [160 x i8], align 8
  %r = call i32 @stat(i8* %path, i8* %st)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %okbb, label %err
err:
  ret i64 -1
okbb:
  %p = getelementptr i8, i8* %st, i64 96
  %pv = bitcast i8* %p to i64*
  %v = load i64, i64* %pv
  ret i64 %v
}
define internal i64 @__rlyeh_file_mtime(i8* %path) {
entry:
  %st = alloca [160 x i8], align 8
  %r = call i32 @stat(i8* %path, i8* %st)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %okbb, label %err
err:
  ret i64 -1
okbb:
  %p = getelementptr i8, i8* %st, i64 48
  %pv = bitcast i8* %p to i64*
  %v = load i64, i64* %pv
  ret i64 %v
}
define internal i64 @__rlyeh_file_mode(i8* %path) {
entry:
  %st = alloca [160 x i8], align 8
  %r = call i32 @stat(i8* %path, i8* %st)
  %ok = icmp eq i32 %r, 0
  br i1 %ok, label %okbb, label %err
err:
  ret i64 -1
okbb:
  %p = getelementptr i8, i8* %st, i64 4
  %pv = bitcast i8* %p to i16*
  %m16 = load i16, i16* %pv
  %m = zext i16 %m16 to i64
  ret i64 %m
}
"#
        .to_string(),
        _ => r#"
; --- Y1 文件元数据 stub（非 Linux/macOS：无 POSIX stat，返回 -1）---
define internal i64 @__rlyeh_file_size(i8* %path) {
entry:
  ret i64 -1
}
define internal i64 @__rlyeh_file_mtime(i8* %path) {
entry:
  ret i64 -1
}
define internal i64 @__rlyeh_file_mode(i8* %path) {
entry:
  ret i64 -1
}
"#
        .to_string(),
    }
}
