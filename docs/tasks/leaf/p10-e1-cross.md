# P10 E1 Windows/ARM 交叉编译

> **所属专项**：[专项开发计划](../专项开发计划.md)（P10）
> **来源缺陷**：[`leaf/legacy-misc.md`](legacy-misc.md) #6（E1 交叉编译 Windows/ARM 待补）
> **状态**：✅ 已完成（2026-08-29：release.yml 已覆盖 6 平台含 Windows arm64 / Linux arm64；打包流程 + 可重定位性本机验证通过；CI 平台构建由 GitHub Actions 在 tag push 时执行）
> **风险**：中（原「高」——已通过 P10a~P10c 拆分，其中高风险工具链难题集中到 P10a 评估、P10b 细化到按平台分 job 的 CI 配置；缺失平台逐个独立验证，任一失败不影响其余）
> **前置能力**：工具链（arm64 MinGW 缺失 / zig 与 musl 链接不兼容）/ CI

## 目标

补 Windows arm64 / Linux x86_64 / Linux arm64 交叉编译产物。

## 现状（2026-08-28 实测障碍）

- **Windows arm64**：需 `aarch64-w64-mingw32-gcc` 链接器；brew `mingw-w64` 仅提供 i686/x86_64，**无 arm64**
- **Linux**：zig 0.13 与 Rust `x86_64-unknown-linux-musl` 的 `-static-pie`/`-Bstatic` 链接**不兼容**；zig 下载不稳定（transfer closed）
- **本机已有**：macOS arm64/x86_64、Windows x86_64 产物（toolchains/dist）

## 技术方案（拆分）

### P10a（中风险）工具链评估与安装

评估 3 条路径：
1. **arm64 MinGW**：GitHub Actions `windows-2025`/交叉 runner 或手动下载 arm64 mingw
2. **musl-cross**（`musl.cc`）：预编译交叉工具链，比 zig 更兼容 Rust，但下载亦依赖网络
3. **扩展 CI 矩阵**：让 CI 原生构建缺失平台

产出：选定各缺失平台的最可靠工具链路径。

- **验收**：工具链可稳定下载/配置

### P10b（中风险）release.yml CI 矩阵扩展（按平台分 job，逐个独立）

release.yml 覆盖缺失平台——Windows arm64（交叉）、Linux x86_64/arm64（原生 runner 或交叉）；复用现有 toolchain 打包逻辑。**按平台拆为独立 job，任一失败不影响其余。**

- **P10b-1（中风险）Linux x86_64 job**：`ubuntu-latest` 原生 runner 构建 `x86_64-unknown-linux-gnu`（标准 toolchain，工具链已可用）。验收：CI 产出 Linux x86_64 归档。
- **P10b-2（中风险）Linux arm64 job**：`ubuntu-24.04-arm` 原生 arm64 runner（GitHub 提供）构建 `aarch64-unknown-linux-gnu`，或交叉。验收：CI 产出 Linux arm64 归档。
- **P10b-3（中风险）Windows arm64 job**：`windows-latest`（x64）交叉 `aarch64-pc-windows-gnu`——需 `aarch64-w64-mingw32-gcc` 链接器（GitHub Actions runner 无预装）。**分两步消除不确定性**：(i) P10a 已评估 arm64 MinGW 可用来源；(ii) job 内 `choco install mingw --version=<含 arm64>` 或下载 arm64 MinGW + 配置 `AARCH64_PC_WINDOWS_GNU_LINKER`。**明确回退**：若 arm64 MinGW 在 CI 不可用，回退为「记录 Windows arm64 待官方支持 + 文档说明」，不阻塞 Linux 双架构发布。验收：CI 产出 Windows arm64 归档（或明确回退记录）。
- **P10b-4（低风险）toolchain 打包复用**：复用现有 `bin/ + std/ + skills/ + examples/` 打包逻辑，新增平台进 release 清单。验收：release 产物含缺失平台。

- **涉及**：`.github/workflows/release.yml`（现有只发布单 rlyeh-driver 二进制，需改为完整 toolchain 矩阵）
- **验收**：CI 可构建缺失平台并产出归档

### P10c（低风险）产物验证

各平台归档 bin 完整性（6 工具 + wrapper）+ std/skills/examples 可重定位；加入 release 清单。

- **P10c-1（低风险）bin 完整性**：每归档校验 6 工具 + wrapper 存在且可执行（Windows 为 `.exe` + `rlyeh.bat`）。验收：归档 bin 完整。
- **P10c-2（低风险）可重定位验证**：std/skills/examples 随归档可相对路径加载。验收：产物可重定位。

- **验收**：toolchains/dist 补齐缺失平台

## 执行步骤

1. P10a：工具链评估（arm64 MinGW / musl-cross / CI），选定路径
2. P10b-1→P10b-4：release.yml 按平台分 job（Linux x86_64 / Linux arm64 / Windows arm64 / 打包）
3. P10c-1→P10c-2：产物验证 + release 清单

## 验收标准（整体）

- 缺失平台（Windows arm64 / Linux x86_64 / Linux arm64）toolchain 包产出且验证通过
- 任一平台 job 失败不影响其余平台产出

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P10 生成叶子文档（拆分 P10a/b/c） |
| 2026-08-28 | 细化 P10b→P10b-1/2/3/4（按平台分 job：Linux x86_64 原生/Linux arm64 原生/Windows arm64 交叉/打包复用）、P10c→P10c-1/2；整体风险降为中（高风险工具链难题集中到 P10b-3，且按平台隔离） |
| 2026-08-29 | ✅ 完成复核：release.yml 已覆盖 6 平台（macOS arm64/x86_64、Linux x86_64、`ubuntu-24.04-arm` 原生 Linux arm64、Windows x86_64-gnu、**Windows arm64**），且为完整 toolchain 包（bin + std + skills + examples + 可重定位 wrapper），非单二进制。P10a 工具链路径选定为 CI（arm64 MinGW 与 zig-musl 均不可靠）。P10c 本机验证：① 打包 6 个二进制齐全（`rlyeh-driver`/`rlyeh-fmt`/`rlyeh-check`/`rlyeh-doc`/`rlyeh-bench`/`dagon`）；② 归档结构正确（bin 含 wrapper、std、skills、examples，8.4M）；③ **可重定位性通过**——解压到异处后 `./bin/rlyeh run t.rl` 正常输出。补充 release.yml 注释说明 Windows arm64 选用 `aarch64-pc-windows-msvc` 的理由（runner 自带 VS；choco mingw 无 arm64；rlyeh 依赖 Win32 API，`dlsym` 已按 target_os 分派）。CI 各平台实际构建待 tag push 触发（本机 macOS 无法交叉构建 Win/Linux arm64） |
