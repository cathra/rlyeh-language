# E3 发布流程

> **所属阶段**：阶段 E
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

dagon publish 重复版本保护 + `rlyeh publish` CLI + release.yml 四平台 + CHANGELOG。

## 背景

阶段 阶段 E 子任务，详见 阶段详情文档 [`stages/E.md`](../../stages/E.md)。

## 技术细节

`dagon publish` 重复版本保护（`PackageIndex::find` 命中即报错）；`rlyeh publish` CLI（委托 `cmd_publish`，`--registry`/`--verbose`，重复版本拦截 exit=1）；release.yml Windows x86_64（`x86_64-pc-windows-msvc` + `.exe` + `shell: bash`）矩阵 4 平台；`CHANGELOG.md` v0.1.0。

## 验证

0.1.0 发布 → 重复发布报错 → 提升 0.2.0 再发布成功。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
