# 工具链使用手册（Manual）

Rlyeh 工具链全部命令的参考手册与常用工作流。

## 1. `rlyeh` 编译器命令

```
Rlyeh 编译器（MVP）
用法:
  rlyeh run <file.rl> [--cache-dir <dir>] [--force] [--no-std] [--verbose] 编译并运行
  rlyeh build <file.rl> [-o <out>] [--cache-dir <dir>] [--force] [--no-std] [--verbose] [--target <triple>] 编译为可执行文件（--target 交叉编译 / wasm32-wasi 生成 .wasm）
  rlyeh test [<tests-dir>] 运行 tests/ 目录用例（compile-pass/compile-fail/run-pass）
  rlyeh fmt <file.rl> [--check] [-w|--write] [--indent N] 格式化代码（默认输出到 stdout）
  rlyeh check <file.rl> 静态分析（未使用变量/恒常条件/冗余比较/不可达代码）
  rlyeh doc <file.rl> [--out <file.md>] [--title <标题>] 提取 /// 注释生成 Markdown 文档
  rlyeh bench <file.rl> [-o <out>] [--runs N] [--warmup N] 编译并基准计时
  rlyeh new <name> [--lib] 创建新项目脚手架（Rlyeh.toml + src/main.rl 或 lib.rl）
  rlyeh publish [--registry <URL>] [--verbose] 打包发布到 dagon 注册表
  rlyeh lsp 启动语言服务器（LSP over stdio，诊断推送）
  rlyeh profile <file.rl_profile> [--out <report.md>] PGO 画像 → 区域大小预测报告
  rlyeh --version 版本信息
```

### 1.1 子命令速查

| 子命令 | 用途 | 典型用法 |
|--------|------|----------|
| `run` | 编译并运行 | `rlyeh run main.rl` |
| `build` | 编译为可执行文件 | `rlyeh build main.rl -o app` |
| `test` | 运行测试用例目录 | `rlyeh test tests/` |
| `fmt` | 格式化代码 | `rlyeh fmt src/main.rl -w` |
| `check` | 静态分析 | `rlyeh check src/main.rl` |
| `doc` | 从 `///` 注释生成文档 | `rlyeh doc lib.rl --out api.md` |
| `bench` | 基准计时 | `rlyeh bench fib.rl --runs 5` |
| `new` | 项目脚手架 | `rlyeh new myapp` |
| `publish` | 发布到注册表 | `rlyeh publish` |
| `lsp` | 语言服务器 | 编辑器集成用 |
| `profile` | PGO 画像分析 | `rlyeh profile app.rl_profile` |

## 2. 独立工具

| 工具 | 用法 |
|------|------|
| `rlyeh-fmt` | `usage: rlyeh-fmt [--check] [-w\|--write] [--indent N] <file>` |
| `rlyeh-check` | `usage: rlyeh-check <file>` |
| `rlyeh-doc` | `用法: rlyeh-doc <file.rl> [--out <file.md>] [--title <标题>]` |
| `rlyeh-bench` | `用法: rlyeh-bench <file.rl \| 可执行文件> [--runs N] [--warmup N] [--out <路径>] [--quiet]` |

> 注：`rlyeh-fmt`/`rlyeh-check`/`rlyeh-doc` 接受文件名作为参数（非 `--help` 风格）；`rlyeh-bench` 支持 `-h/--help`。

## 3. `dagon` 包管理器

```
Dagon 是 Rlyeh 语言的包管理器：项目脚手架、依赖解析、注册表发布与构建集成。

Commands:
  new      创建新项目
  init     初始化当前目录为项目
  add      添加依赖（name[@req]，如 foo@^1.0）
  remove   移除依赖
  build    编译项目
  run      编译并运行（程序参数需以 -- 分隔：dagon run -- --flag x）
  test     构建并运行测试（tests/*.rl）
  update   重新解析依赖并更新 Rlyeh.lock
  publish  打包并发布到注册表
  search   在注册表中搜索包
  clean    清理 target 目录

Options:
      --verbose                详细输出
      --registry <URL>         注册表地址（默认 ~/.rl/registry；支持路径或 http://）
```

### 3.1 注册表

- 默认注册表：`~/.rl/registry`（本地目录注册表，`rlyeh publish`/`dagon search` 自动使用）
- 指定注册表：`dagon --registry /path/to/reg` 或 `rlyeh publish --registry file:///path/to/reg`
- 包存储：`pkgs/<name>-<ver>.tar.gz` + `index/<name>.json`

## 4. 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `RLYEH_STD_PATH` | `~/.rl/std`（wrapper 自动注入） | 标准库目录（含 `core.rl`） |
| `RLYEH_PREFIX` | `$HOME/.rl` | 工具链安装前缀（install.sh 使用） |
| `PATH` | — | 需包含 `$HOME/.rl/bin` 才能直接使用 `rlyeh` |

## 5. rlyeh-language 技能（CodeBuddy Skill）

工具链随附 `rlyeh-language` 技能（`SKILL.md` + `references/`），供 CodeBuddy 等 IDE 加载为项目级技能，辅助编写 / 审查 / 调试 Rlyeh 代码：

- **安装位置**：`<prefix>/skills/rlyeh-language/`（默认 `~/.rl/skills/rlyeh-language/`，install.sh 自动复制）
- **归档包含**：`rlyeh-toolchain-<ver>-<os>-<arch>.tar.gz` 内含 `skills/` 目录，解压后即可使用
- **内容**：语法 / 语义 / 标准库 / MVP 陷阱速查（`references/language.md` / `semantics.md` / `std-lib.md` / `pitfalls.md`）
- **使用**：IDE 将 `<prefix>/skills/` 注册为技能目录后自动发现；更新工具链后重载技能即可同步

## 6. 常用工作流

### 5.1 Hello World

```bash
rlyeh run examples/hello-world.rl
```

### 5.2 创建并开发项目

```bash
rlyeh new myapp && cd myapp     # 或 dagon init
rlyeh run src/main.rl         # 开发迭代
rlyeh build src/main.rl -o app
./app
```

### 5.3 库项目

```bash
rlyeh new mylib --lib           # 生成 lib.rl
rlyeh doc lib.rl --out docs/api.md
rlyeh publish                   # 发布到本地注册表
```

### 5.4 依赖管理

```bash
dagon add foo@^1.0               # 添加依赖（解析到 Rlyeh.lock）
dagon remove foo
dagon update                     # 重新解析
dagon build && dagon run
```

### 5.5 格式化与检查（CI 推荐）

```bash
rlyeh fmt src/main.rl --check   # 检查格式（不写回）
rlyeh check src/main.rl         # 静态分析
```

### 5.6 基准测试

```bash
rlyeh-bench fib.rl --runs 10 --warmup 2
# 或
rlyeh bench fib.rl --runs 5
```

### 5.7 LSP（编辑器集成）

```bash
rlyeh lsp     # LSP over stdio，诊断推送；配合 rlyeh-lsp crate 使用
```

## 7. 示例项目导航

仓库 `examples/` 下已有大量可运行示例：

| 示例 | 说明 |
|------|------|
| `examples/hello-world.rl` | 入门 |
| `examples/arith-print.rl` | 算术与打印 |
| `examples/projects/raytracer/` | 光线追踪 |
| `examples/projects/chatd/` | NIO 聊天室（actor） |
| `examples/projects/wordfreq/` | 词频统计（HashMap/sort） |
| `examples/projects/benchmarks/` | 跨语言性能对比基准 |
