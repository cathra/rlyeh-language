# Rlyeh 标准库详述

> 最后更新：2026-08-31
>
> 本目录对标准库**每个类型**逐一说明：类型定位、构造、成员、方法（签名 + 语义 + 用例）。每个类型页含 `> **C 程序员对照**` 块，帮有 C 基础的读者理解"Rlyeh 标准库类型对应 C 里怎么写、解决了什么隐患"。
> 标准库采用模块化拆分：`rlyeh-std/rlyeh/module.rl` 声明与导出入口 + `core/module.rl`（根类型）+ 平级平铺单元 `str_ext/` / `convert/` / `collections/` / `externs/` + 子模块 `time/` / `io/` / `net/` / `sync/`，裸名即用。
> 权威 API 总表见 [../11-stdlib.md](../11-stdlib.md)；规划中模块见 [../../std-lib.md](../../std-lib.md)。

## 类型索引

**容器与值类型**
- [String](./string.md)
- [Vec](./vec.md)
- [HashMap](./hashmap.md)
- [Option](./option.md)
- [Result](./result.md)

**智能指针**
- [Box / Rc / Arc / Gc](./smart-pointers.md)

**IO 与文件系统**
- [File / Path / fs / sendfile](./io.md)

**网络**
- [TcpStream / TcpListener / HttpClient / UdpSocket](./net.md)

**并发与同步**
- [Mutex / RwLock / Condvar / Channel / Thread](./sync.md)

**异步**
- [Future / Poll / Context](./future.md)

**时间**
- [Duration / Instant / SystemTime](./time.md)

**序列化**
- [json](./json.md)
- [toml](./toml.md)

> 说明：本 MVP 中 `i8` 与 `u8` 在底层共享 64 位整数槽；`char` 为 32 位 Unicode 码点；字符串字面量 / `&str` 与 `String` 自动升级互通。
