#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Rlyeh 差分 / 快照测试 harness（SH-P2-5 K4/K5，0.2.0 PoC）。

对一组 `.rl` 用例，分别经「参考编译器」（当前为 Rust 实现的 `rlyeh`）编译，
捕获四个维度产物并与 golden 快照比对：

    ir    : `rlyeh build --emit ir`    → LLVM IR 文本
    ast   : `rlyeh build --emit ast`   → AST 文本（`{:#?}`）
    hir   : `rlyeh build --emit hir`   → HIR 文本（`{:#?}`）
    ast-user / hir-user : 仅用户源码项（排除 std 前缀），体积小、适合快照基线
    run   : `rlyeh run`                → 程序标准输出（行为）
    diagnostics : `rlyeh run <file>` 的 stderr（类型检查 / 借用检查诊断文本，
                 SH-P2-6 L3 诊断对拍的 harness 侧；编译失败文件在类型检查阶段
                 即中止，stderr 即诊断文本，无论退出码均记为已获取）

0.2.0 内为「单编译器快照对拍」：以 Rust 编译器自身产物作参考快照。
当未来 Rlyeh 自写编译器就位，可用 `--rlyeh-b <另一编译器>` 触发「双编译器差分」
（同维度产物逐文件比对，无需快照基线），管道即无缝升级。

用法:
    # 比对模式（默认）：与已有快照比对，任一维度差异即失败
    python3 scripts/diff_harness.py check  [dirs...]
    # 生成 / 更新快照基线
    python3 scripts/diff_harness.py update [dirs...]

    # 可选：指定维度子集（默认 ir,ast,hir,run）
    --dims ir,ast,hir,run
    # 可选：基线清单文件（每行一个 .rl 相对路径，# 开头为注释；优先于 dirs）
    --manifest tests/snapshot-baseline.txt
    # 可选：IR 探针模式——等价于 `--manifest <IR探针清单> --dims ir`；
    # 清单见 tests/snapshot-baseline-ir-probe.txt（小子集，存 IR 快照）
    --probe
    # 可选：指定编译器二进制（默认 ./target/debug/rlyeh）
    --rlyeh /path/to/rlyeh
    # 可选：第二编译器，启用双编译器差分（逐维度比对两份产物，不读快照）
    --rlyeh-b /path/to/rlyeh2
    # 可选：快照根目录（默认 tests/snapshots）
    --snap-dir tests/snapshots
    # 可选：单文件超时秒数（默认 30）
    --timeout 30

注: AST/HIR/IR 文本被 std 前缀主导（单程序 AST 可达 ~21 万行），逐程序存储
不现实；因此 0.2.0 的存储式快照基线以 `run`（运行行为，体积小、语义信号强）
为主，AST/HIR/IR 更适合作为未来双编译器 `--rlyeh-b` 实时差分维度。

退出码: 0 = 全部通过；1 = 存在差异 / 错误；2 = 用法错误。
"""
import argparse
import os
import subprocess
import sys

DIMS = ["ir", "ast", "hir", "ast-user", "hir-user", "run", "diagnostics"]
DEFAULT_RLYEH = os.path.join("target", "debug", "rlyeh")
DEFAULT_SNAP_DIR = os.path.join("tests", "snapshots")
# IR 探针基线清单：精选「小子集」用例（见下），因单程序 IR 被 std 前缀主导
# （~1.8 万行），全量存储不现实；探针仅对极少数代表性程序存 IR 快照，
# 用于捕获编译器 IR 级回归 / 非确定性。
DEFAULT_PROBE_MANIFEST = os.path.join("tests", "snapshot-baseline-ir-probe.txt")
TIMEOUT = 30


def run_cmd(rlyeh, args, timeout):
    """运行 rlyeh 子命令，返回 (returncode, stdout, stderr)。"""
    try:
        p = subprocess.run(
            [rlyeh, *args],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return 124, "", f"timeout after {timeout}s"
    except FileNotFoundError as e:
        return 127, "", f"binary not found: {e}"


def capture(rlyeh, path, timeout, dims):
    """捕获单文件指定维度产物，返回 {dim: (ok, text)}。

    dims 为 {"ir","ast","hir","run","diagnostics"} 的子集。ok 表示该维度
    成功获取（编译/运行成功）。run 维度对编译失败文件记为失败；
    diagnostics 维度捕获 `rlyeh run` 的 stderr（类型检查 / 借用检查诊断），
    无论退出码均记为已获取（诊断文本本身即回归标的）。
    """
    out = {}
    for dim in dims:
        if dim == "diagnostics":
            # 编译失败文件在类型检查阶段即中止，stderr 即诊断文本；
            # 视为已获取（ok=True），诊断文本本身即快照标的。
            rc, so, se = run_cmd(rlyeh, ["run", path], timeout)
            out[dim] = (True, se)
        elif dim == "run":
            rc, so, se = run_cmd(rlyeh, ["run", path], timeout)
            out[dim] = (rc == 0, so if rc == 0 else se)
        else:
            rc, so, se = run_cmd(rlyeh, ["build", path, "--emit", dim], timeout)
            out[dim] = (rc == 0, so if rc == 0 else se)
    return out


def snap_path(snap_dir, rel, dim):
    base = os.path.splitext(rel)[0]
    return os.path.join(snap_dir, base, f"{dim}.txt")


def normalize(text):
    return "\n".join(line.rstrip() for line in text.splitlines()).rstrip() + "\n"


def do_update(rlyeh, files, snap_dir, timeout, dims):
    total = 0
    for path in files:
        rel = path
        captured = capture(rlyeh, path, timeout, dims)
        for dim, (ok, text) in captured.items():
            if not ok:
                print(f"[SKIP] {rel} [{dim}] 获取失败（编译/运行错误），不写快照")
                continue
            sp = snap_path(snap_dir, rel, dim)
            os.makedirs(os.path.dirname(sp), exist_ok=True)
            with open(sp, "w", encoding="utf-8") as f:
                f.write(normalize(text))
            total += 1
    print(f"快照更新完成: {total} 个维度文件写入 {snap_dir}")
    return 0


def do_check(rlyeh, files, snap_dir, timeout, rlyeh_b=None, dims=None):
    pass_n = 0
    diff_n = 0
    miss_n = 0
    err_n = 0
    rows = []

    def compare_dims(captured_a, captured_b=None):
        """比对维度。captured_b 为 None 时与快照比对；否则双编译器差分。"""
        nonlocal pass_n, diff_n, miss_n, err_n
        for dim, (ok, text) in captured_a.items():
            if not ok:
                err_n += 1
                rows.append(f"  [ERR ] {dim}: 参考编译器获取失败")
                continue
            if captured_b is not None:
                ok_b, text_b = captured_b[dim]
                if not ok_b:
                    err_n += 1
                    rows.append(f"  [ERR ] {dim}: 第二编译器获取失败")
                    continue
                if normalize(text) == normalize(text_b):
                    pass_n += 1
                else:
                    diff_n += 1
                    rows.append(f"  [DIFF] {dim}: 两份产物不一致")
                continue
            # 单编译器：与快照比对
            sp = snap_path(snap_dir, rel, dim)
            if not os.path.exists(sp):
                miss_n += 1
                rows.append(f"  [MISS] {dim}: 无快照（先 `update`）")
                continue
            with open(sp, "r", encoding="utf-8") as f:
                snap = f.read()
            if normalize(text) == snap:
                pass_n += 1
            else:
                diff_n += 1
                rows.append(f"  [DIFF] {dim}: 与快照不一致")

    for path in files:
        rel = path
        captured_a = capture(rlyeh, path, timeout, dims)
        if rlyeh_b:
            captured_b = capture(rlyeh_b, path, timeout)
            rows.append(f"[{rel}]")
            compare_dims(captured_a, captured_b)
            rows.append("")  # 分隔
        else:
            rows.append(f"[{rel}]")
            compare_dims(captured_a)
            rows.append("")

    for r in rows:
        if r:
            print(r)
    print(
        f"快照/差分汇总: 通过 {pass_n}, 差异 {diff_n}, 缺快照 {miss_n}, 错误 {err_n}"
    )
    return 1 if (diff_n or err_n) else 0


def collect_rl(paths):
    files = []
    for p in paths:
        if os.path.isfile(p) and p.endswith(".rl"):
            files.append(os.path.relpath(p))
        elif os.path.isdir(p):
            for root, _, names in os.walk(p):
                for n in sorted(names):
                    if n.endswith(".rl"):
                        files.append(os.path.relpath(os.path.join(root, n)))
    return files


def read_manifest(path):
    """读取基线清单：每行一个 .rl 相对路径；空行与 `#` 开头注释忽略。"""
    files = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            files.append(line)
    return files


def main():
    ap = argparse.ArgumentParser(description="Rlyeh 差分/快照 harness")
    ap.add_argument("mode", choices=["check", "update"], help="check=比对, update=写快照")
    ap.add_argument("dirs", nargs="*", default=["tests/run-pass", "examples"],
                    help="待扫描的 .rl 目录（默认 tests/run-pass 与 examples）")
    ap.add_argument("--dims", default="ir,ast,hir,run",
                    help="维度子集，逗号分隔（默认 ir,ast,hir,run）")
    ap.add_argument("--manifest", default=None,
                    help="基线清单文件（每行一个 .rl 路径，# 注释；优先于 dirs）")
    ap.add_argument("--probe", action="store_true",
                    help="IR 探针模式：等价于 --manifest <IR探针清单> --dims ir")
    ap.add_argument("--rlyeh", default=DEFAULT_RLYEH)
    ap.add_argument("--rlyeh-b", default=None, help="第二编译器（启用双编译器差分）")
    ap.add_argument("--snap-dir", default=DEFAULT_SNAP_DIR)
    ap.add_argument("--timeout", type=int, default=TIMEOUT)
    args = ap.parse_args()

    # --probe：默认值改写（用户显式 --dims / --manifest 时尊重其设定）
    if args.probe and args.dims == "ir,ast,hir,run":
        args.dims = "ir"

    dims = [d.strip() for d in args.dims.split(",") if d.strip()]
    bad = [d for d in dims if d not in DIMS]
    if bad:
        print(f"未知维度: {bad}（可选: {DIMS}）")
        return 2

    # --probe：未显式指定 manifest 时套用 IR 探针默认清单
    if args.probe and not args.manifest:
        args.manifest = DEFAULT_PROBE_MANIFEST

    if args.manifest:
        files = read_manifest(args.manifest)
    else:
        files = collect_rl(args.dirs)
    if not files:
        print(f"未找到 .rl 文件: {args.dirs}")
        return 2

    if args.mode == "update":
        return do_update(args.rlyeh, files, args.snap_dir, args.timeout, dims)
    return do_check(args.rlyeh, files, args.snap_dir, args.timeout, args.rlyeh_b, dims)


if __name__ == "__main__":
    sys.exit(main())
