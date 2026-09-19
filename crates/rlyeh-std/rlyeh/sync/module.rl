// sync/module.rl：并发原语模块根——只做「子模块声明 + 暴露内容导出」（2026-09-19 重整）。
//
// 组成（拆分前全部内联于本文件，现下沉独立文件，由 `module X;` 声明 +
// `pub import` 重导出，保持 `sync::Mutex` 等原全名可见）：
//   sync/mutex.rl      MutexGuard<T> / Mutex<T>        → 子模块 sync::mutex
//   sync/rwlock.rl     RwLock<T> / RwLockReadGuard<T> / RwLockWriteGuard<T> → sync::rwlock
//   sync/atomic.rl     Ordering / AtomicI64           → sync::atomic
//   sync/condvar.rl    Condvar                        → sync::condvar
//   sync/barrier.rl    Barrier                        → sync::barrier
//   sync/channel.rl    Channel/Sender/Receiver/ChannelPair + 错误类型 + RecvAsync
//                       + channel()/bounded_channel() + impl Sender/Receiver → sync::channel
//
// 声明顺序 = 收集期注册顺序：mutex / rwlock / atomic 先于 condvar / barrier /
// channel（channel 的 RecvAsync::poll 引用 future::poll::Context、Sender::send
// 引用 net::send_all，均经重导出别名或后缀兜底解析；sync 内部互引用如
// `sync::Mutex` 经 `pub import` 重导出保名）。

module mutex;
pub import mutex::Mutex;
pub import mutex::MutexGuard;
module rwlock;
pub import rwlock::RwLock;
pub import rwlock::RwLockReadGuard;
pub import rwlock::RwLockWriteGuard;
module atomic;
pub import atomic::AtomicI64;
pub import atomic::Ordering;
module condvar;
pub import condvar::Condvar;
module barrier;
pub import barrier::Barrier;
module channel;
pub import channel::Channel;
pub import channel::Sender;
pub import channel::Receiver;
pub import channel::ChannelPair;
pub import channel::SendError;
pub import channel::RecvError;
pub import channel::TryRecvError;
pub import channel::RecvAsync;
