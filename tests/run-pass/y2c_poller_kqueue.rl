// Y2c（lang-defects #7）：Poller 完整 kqueue 分派端到端验证
// Poller::new() 在 macOS/BSD 建 kqueue（kq>0），register→EV_ADD，poll→kq_poll；
// 其他平台 kq=-1 回退 poll(2)。公共 API 跨平台一致，本测试端到端验证分派。
// socketpair：fd1 写数据 → fd0 可读 → poll 返回 token + Readable 就绪事件。
//
// 注：跨模块调用 io::nio::Poller 的 &mut self 方法（register）时，自动借用 &mut
// 已生效（lang-defects #10，2026-09-06 修复），故直接写 `q.register(...)` 即可，
// 不再需要显式 (&mut q) 借用。kqueue 路径本身（EV_ADD / kq_poll 分派）已正确工作。

fn main() {
    match Poller::new() {
        Result::Ok(p) => {
            let mut q = p;
            let sp = net::socketpair_stream();
            let fd0 = net::fd_at(sp, 0);
            let fd1 = net::fd_at(sp, 1);
            match q.register(fd0, 7, io::nio::Interest::Readable) {
                Result::Ok(_) => {}
                Result::Err(_) => { println(-1); return; }
            }
            let _ = net::send_all(fd1, String::from("x"));
            match q.poll(-1) {
                Result::Ok(events) => {
                    let mut ok = 0;
                    if events.len() >= 1 { ok = ok + 1; }
                    if events.len() >= 1 && events[0].token == 7 { ok = ok + 1; }
                    if events.len() >= 1 && events[0].is_readable() { ok = ok + 1; }
                    println(ok); // 期望 3（1 个就绪事件 / token=7 / 可读）
                }
                Result::Err(_) => { println(-2); }
            }
        }
        Result::Err(_) => { println(-3); }
    }
}
