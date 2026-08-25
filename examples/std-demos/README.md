# Zeta 标准库用例项目（std-demos）

按 **std 标准库功能点**组织的可运行用例项目集合。每个目录对应一个功能点，
源码直接取自 `tests/run-pass` 已通过全量回归验证的用例，保证可编译、可运行、
输出确定。

## 目录索引

| 目录 | 功能点 | std-lib 章节 | 用例数 |
|------|--------|--------------|--------|
| [01-option-result](01-option-result/) | Option / Result / `?` / Error trait | §2.1/2.2, §12 | 3 |
| [02-collections](02-collections/) | Vec / String / HashMap / str | §3 | 4 |
| [03-iterator](03-iterator/) | 迭代器与适配器（map/filter/fold/...） | §2.3 | 4 |
| [04-file-io](04-file-io/) | File / 路径 / 文件系统 / stdio | §4 | 8 |
| [05-networking](05-networking/) | TCP / SocketAddr | §5 | 2 |
| [06-sync](06-sync/) | Mutex / Channel | §6 | 2 |
| [07-time](07-time/) | Duration / Instant / sleep | §7, §10.2 | 1 |
| [08-format](08-format/) | 格式化宏 / Display / Debug | §8 | 2 |
| [09-serialization](09-serialization/) | JSON / TOML / serde derive | §9 | 4 |
| [10-async](10-async/) | async fn / await / block_on / join_all / timeout | §10 | 6 |
| [11-smart-pointers](11-smart-pointers/) | Box / Rc / Arc / Gc | §11 | 4 |
| [12-macros](12-macros/) | 集合宏 / macro_rules! | §3.9a | 3 |

## 构建工具链

```bash
cargo build --release -p zeta-driver
# 产物：target/release/zeta
```

## 运行单个用例

```bash
zeta run examples/std-demos/<目录>/<文件>.zeta
```

也可一次跑完所有纯计算用例：

```bash
for f in examples/std-demos/0*/[0-9]*.zeta; do
  # 跳过需交互/常驻的用例（stdin_enhance、tcp_echo）
  case "$f" in *stdin_enhance*|*tcp_echo*) continue;; esac
  echo "== $f =="; zeta run "$f"
done
```

## 特殊用例说明

- `04-file-io/stdin_enhance.zeta`：需标准输入，如 `echo "hello" | zeta run ...`
- `05-networking/tcp_echo.zeta`：常驻 TCP echo 服务器，Ctrl-C 退出
- 其余用例均为纯计算 / 有限 IO，直接运行即可

## 与测试体系的关系

本目录用例与 `tests/run-pass` 同源（一份代码、两处用途）。回归验证统一走：

```bash
cargo test --workspace -- --test-threads=1
zeta test tests
```

> 新增 std 功能点用例时：先落地 `tests/run-pass/<name>.zeta` 通过回归，
> 再同步复制到本目录对应功能点，保证示例始终可运行。
