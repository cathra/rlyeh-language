# 同步原语（sync 模块）

线程同步与并发原语，位于 `sync` 子模块（P1–P3 ✅ 已实现）。底层使用 pthread 锁与条件变量。

## Mutex

互斥锁（独占锁）。

### 构造

```rlyeh
let m = Mutex::new();
```

- `Mutex::new() -> Mutex`

### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `lock` | `() -> ()` | 加锁（阻塞直到获取） |
| `try_lock` | `() -> bool` | 尝试加锁，成功 `true` 失败 `false`（非阻塞） |
| `unlock` | `() -> ()` | 解锁 |

```rlyeh
let m = Mutex::new();
m.lock();
// 临界区
m.unlock();
if m.try_lock() {
    m.unlock();
}
```

## RwLock

读写锁（多读单写）。

- `RwLock::new() -> RwLock`
- `read_lock() -> ()` / `read_unlock() -> ()`：读锁（共享）
- `write_lock() -> ()` / `write_unlock() -> ()`：写锁（独占）

```rlyeh
let rw = RwLock::new();
rw.read_lock();
rw.read_unlock();
rw.write_lock();
rw.write_unlock();
```

## Condvar

条件变量（配合 Mutex 使用）。

- `Condvar::new() -> Condvar`
- `wait(mutex: Mutex) -> ()`：释放锁并等待通知
- `notify_one() -> ()`：唤醒一个等待者
- `notify_all() -> ()`：唤醒全部等待者

```rlyeh
let cv = Condvar::new();
let m = Mutex::new();
m.lock();
cv.wait(m);          // 释放 m 并等待
m.unlock();
cv.notify_one();
```

## Barrier

屏障（N 个线程到齐后放行）。

- `Barrier::new(n: i64) -> Barrier`
- `wait() -> ()`：阻塞直到 n 个线程都调用 `wait`

```rlyeh
let b = Barrier::new(2);
b.wait();            // 第 2 个线程到齐后双双放行
```

## Channel

线程间消息通道（MPSC 风格）。

- `Channel::new() -> (Sender<T>, Receiver<T>)`
- `Sender::send(v: T) -> ()`
- `Receiver::recv() -> T`

```rlyeh
let (tx, rx) = Channel::new();
tx.send(42);
let v = rx.recv();   // 42
```

## Thread

### 自由函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `Thread::start` | `(f: fn(..), args) -> ()` | 启动新线程 |
| `Thread::join` | `(t) -> ()` | 等待线程结束 |
| `Thread::sleep` | `(ms: i64) -> ()` | 休眠毫秒 |
| `Thread::join_all` | `(threads) -> ()` | 等待全部线程（Y8 ✅） |

```rlyeh
Thread::sleep(100);
```

### Builder

```rlyeh
let b = thread::Builder::new();
b.name(String::from("worker"));
b.stack_size(8192);
b.spawn(worker_fn);
```

## 完整示例

```rlyeh
fn main() {
    let m = Mutex::new();
    m.lock();
    // 临界区操作共享状态
    m.unlock();

    let (tx, rx) = Channel::new();
    tx.send(String::from("hi"));
    let v = rx.recv();
    println(v);
}
```

> 与 Actor 模型对比：sync 原语是传统共享内存并发；Actor（见 [../10-concurrency.md](../10-concurrency.md)）是消息传递并发，二者可混用。

---

[← 返回标准库详述索引](./index.md)
