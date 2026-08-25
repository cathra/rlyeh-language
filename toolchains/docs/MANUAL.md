# 工具链使用手册（Manual）

Zeta 工具链全部命令的参考手册与常用工作流。

## 1. `zeta` 编译器命令

```
Zeta 编译器（MVP）
用法:
  zeta run <file.zeta> [--cache-dir <dir>] [--force] [--no-std] [--verbose] 编译并运行
  zeta build <file.zeta> [-o <out>] [--cache-dir <dir>] [--force] [--no-std] [--verbose] [--target <triple>] 编译为可执行文件（--target 交叉编译 / wasm32-wasi 生成 .wasm）
  zeta test [<tests-dir>] 运行 tests/ 目录用例（compile-pass/compile-fail/run-pass）
  zeta fmt <file.zeta> [--check] [-w|--write] [--indent N] 格式化代码（默认输出到 stdout）
  zeta check <file.zeta> 静态分析（未使用变量/恒常条件/冗余比较/不可达代码）
  zeta doc <file.zeta> [--out <file.md>] [--title <标题>] 提取 /// 注释生成 Markdown 文档
  zeta bench <file.zeta> [-o <out>] [--runs N] [--warmup N] 编译并基准计时
  zeta new <name> [--lib] 创建新项目脚手架（Zeta.toml + src/main.zeta 或 lib.zeta）
  zeta publish [--registry <URL>] [--verbose] 打包发布到 zep 注册表
  zeta lsp 启动语言服务器（LSP over stdio，诊断推送）
  zeta profile <file.zeta_profile> [--out <report.md>] PGO 画像 → 区域大小预测报告
  zeta --version 版本信息
```

### 1.1 子命令速查

| 子命令 | 用途 | 典型用法 |
|--------|------|----------|
| `run` | 编译并运行 | `zeta run main.zeta` |
| `build` | 编译为可执行文件 | `zeta build main.zeta -o app` |
| `test` | 运行测试用例目录 | `zeta test tests/` |
| `fmt` | 格式化代码 | `zeta fmt src/main.zeta -w` |
| `check` | 静态分析 | `zeta check src/main.zeta` |
| `doc` | 从 `///` 注释生成文档 | `zeta doc lib.zeta --out api.md` |
| `bench` | 基准计时 | `zeta bench fib.zeta --runs 5` |
| `new` | 项目脚手架 | `zeta new myapp` |
| `publish` | 发布到注册表 | `zeta publish` |
| `lsp` | 语言服务器 | 编辑器集成用 |
| `profile` | PGO 画像分析 | `zeta profile app.zeta_profile` |

## 2. 独立工具

| 工具 | 用法 |
|------|------|
| `zeta-fmt` | `usage: zeta-fmt [--check] [-w\|--write] [--indent N] <file>` |
| `zeta-check` | `usage: zeta-check <file>` |
| `zeta-doc` | `用法: zeta-doc <file.zeta> [--out <file.md>] [--title <标题>]` |
| `zeta-bench` | `用法: zeta-bench <file.zeta \| 可执行文件> [--runs N] [--warmup N] [--out <路径>] [--quiet]` |

> 注：`zeta-fmt`/`zeta-check`/`zeta-doc` 接受文件名作为参数（非 `--help` 风格）；`zeta-bench` 支持 `-h/--help`。

## 3. `zep` 包管理器

```
Zep 是 Zeta 语言的包管理器：项目脚手架、依赖解析、注册表发布与构建集成。

Commands:
  new      创建新项目
  init     初始化当前目录为项目
  add      添加依赖（name[@req]，如 foo@^1.0）
  remove   移除依赖
  build    编译项目
  run      编译并运行（程序参数需以 -- 分隔：zep run -- --flag x）
  test     构建并运行测试（tests/*.zeta）
  update   重新解析依赖并更新 Zeta.lock
  publish  打包并发布到注册表
  search   在注册表中搜索包
  clean    清理 target 目录

Options:
      --verbose                详细输出
      --registry <URL>         注册表地址（默认 ~/.zeta/registry；支持路径或 http://）
```

### 3.1 注册表

- 默认注册表：`~/.zeta/registry`（本地目录注册表，`zeta publish`/`zep search` 自动使用）
- 指定注册表：`zep --registry /path/to/reg` 或 `zeta publish --registry file:///path/to/reg`
- 包存储：`pkgs/<name>-<ver>.tar.gz` + `index/<name>.json`

## 4. 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `ZETA_STD_PATH` | `~/.zeta/std`（wrapper 自动注入） | 标准库目录（含 `core.zeta`） |
| `ZETA_PREFIX` | `$HOME/.zeta` | 工具链安装前缀（install.sh 使用） |
| `PATH` | — | 需包含 `$HOME/.zeta/bin` 才能直接使用 `zeta` |

## 5. zeta-language 技能（CodeBuddy Skill）

工具链随附 `zeta-language` 技能（`SKILL.md` + `references/`），供 CodeBuddy 等 IDE 加载为项目级技能，辅助编写 / 审查 / 调试 Zeta 代码：

- **安装位置**：`<prefix>/skills/zeta-language/`（默认 `~/.zeta/skills/zeta-language/`，install.sh 自动复制）
- **归档包含**：`zeta-toolchain-<ver>-<os>-<arch>.tar.gz` 内含 `skills/` 目录，解压后即可使用
- **内容**：语法 / 语义 / 标准库 / MVP 陷阱速查（`references/language.md` / `semantics.md` / `std-lib.md` / `pitfalls.md`）
- **使用**：IDE 将 `<prefix>/skills/` 注册为技能目录后自动发现；更新工具链后重载技能即可同步

## 6. 常用工作流

### 5.1 Hello World

```bash
zeta run examples/hello-world.zeta
```

### 5.2 创建并开发项目

```bash
zeta new myapp && cd myapp     # 或 zep init
zeta run src/main.zeta         # 开发迭代
zeta build src/main.zeta -o app
./app
```

### 5.3 库项目

```bash
zeta new mylib --lib           # 生成 lib.zeta
zeta doc lib.zeta --out docs/api.md
zeta publish                   # 发布到本地注册表
```

### 5.4 依赖管理

```bash
zep add foo@^1.0               # 添加依赖（解析到 Zeta.lock）
zep remove foo
zep update                     # 重新解析
zep build && zep run
```

### 5.5 格式化与检查（CI 推荐）

```bash
zeta fmt src/main.zeta --check   # 检查格式（不写回）
zeta check src/main.zeta         # 静态分析
```

### 5.6 基准测试

```bash
zeta-bench fib.zeta --runs 10 --warmup 2
# 或
zeta bench fib.zeta --runs 5
```

### 5.7 LSP（编辑器集成）

```bash
zeta lsp     # LSP over stdio，诊断推送；配合 zeta-lsp crate 使用
```

## 7. 示例项目导航

仓库 `examples/` 下已有大量可运行示例：

| 示例 | 说明 |
|------|------|
| `examples/hello-world.zeta` | 入门 |
| `examples/arith-print.zeta` | 算术与打印 |
| `examples/projects/raytracer/` | 光线追踪 |
| `examples/projects/chatd/` | NIO 聊天室（actor） |
| `examples/projects/wordfreq/` | 词频统计（HashMap/sort） |
| `examples/projects/benchmarks/` | 跨语言性能对比基准 |
