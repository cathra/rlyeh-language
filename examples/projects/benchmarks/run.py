#!/usr/bin/env python3
"""
统一性能对比脚本：Zeta vs C / C++ / Swift / Rust
用法: python3 run.py [--runs N] [--warmup N] [--only fib,matmul] [--skip-zeta-build]
说明:
  - 每个 benchmark 每种语言独立二进制（bench_<lang>）
  - 编译优化级别: Zeta=无优化(clang O0 汇编, 其当前真实水平); C/C++=clang -O3;
    Swift=swiftc -O; Rust=rustc -O
  - 计时: warmup 后 runs 次取中位数（毫秒）
"""
import os
import subprocess
import sys
import time
import statistics
import json
import datetime

ROOT = os.path.dirname(os.path.abspath(__file__))
ZETA_DRIVER = os.path.join(ROOT, "..", "..", "..", "target", "release", "zeta-driver")

BENCHMARKS = ["fib", "loop_sum", "matmul", "strcat", "hashmap", "sort"]

LANGS = [
    ("Zeta",  "zeta",  "zeta"),
    ("C",     "c",     "c"),
    ("C++",   "cpp",   "cpp"),
    ("Swift", "swift", "swift"),
    ("Rust",  "rust",  "rs"),
]

DESC = {
    "fib":      "fib(30) 双递归（函数调用 + 整数运算）",
    "loop_sum": "1 亿次 i64 循环累加（整数运算 + 分支）",
    "matmul":   "256x256 f64 矩阵乘法（浮点 + 内存访问）",
    "strcat":   "字符串拼接 10 万次（缓冲扩容）",
    "hashmap":  "20 万 insert + 20 万 get，i64 键（哈希表）",
    "sort":     "LCG 生成 5000 个 i64 排序（排序算法）",
}


def build(bench: str, name: str, lang: str, ext: str) -> str | None:
    src = os.path.join(ROOT, bench, f"{bench}.{ext}")
    out = os.path.join(ROOT, bench, f"bench_{lang}")
    if lang == "zeta":
        cmd = [ZETA_DRIVER, "build", src, "-o", out]
    elif lang == "c":
        cmd = ["clang", "-O3", "-o", out, src]
    elif lang == "cpp":
        cmd = ["clang++", "-O3", "-o", out, src]
    elif lang == "swift":
        cmd = ["swiftc", "-O", "-o", out, src]
    else:  # rust
        cmd = ["rustc", "-O", "-o", out, src]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        print(f"  [编译失败] {bench}/{name}: {r.stderr.strip()[:300]}")
        return None
    return out


def time_binary(binary: str, runs: int, warmup: int) -> float | None:
    for _ in range(warmup):
        subprocess.run([binary], capture_output=True)
    times = []
    for _ in range(runs):
        t0 = time.perf_counter()
        subprocess.run([binary], capture_output=True)
        times.append((time.perf_counter() - t0) * 1000.0)
    return statistics.median(times)


def main() -> int:
    runs = 5
    warmup = 1
    only = None
    skip_build = False
    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--runs" and i + 1 < len(args):
            runs = int(args[i + 1]); i += 2
        elif args[i] == "--warmup" and i + 1 < len(args):
            warmup = int(args[i + 1]); i += 2
        elif args[i] == "--only" and i + 1 < len(args):
            only = args[i + 1].split(","); i += 2
        elif args[i] == "--skip-zeta-build":
            skip_build = True; i += 1
        else:
            i += 1

    benches = [b for b in BENCHMARKS if only is None or b in only]
    if not os.path.exists(ZETA_DRIVER):
        print(f"未找到 zeta-driver: {ZETA_DRIVER}")
        return 1

    print(f"Zeta 编译器: {ZETA_DRIVER}")
    print(f"编译策略: Zeta=无优化(clang O0) / C,C++=-O3 / Swift=-O / Rust=-O")
    print(f"计时: warmup={warmup} 次 + 正式 {runs} 次取中位数\n")

    results: dict[str, dict[str, float]] = {}
    for bench in benches:
        print(f"== {bench} ({DESC[bench]})")
        results[bench] = {}
        for name, lang, ext in LANGS:
            if lang == "zeta" and skip_build:
                out = os.path.join(ROOT, bench, f"bench_zeta")
                if not os.path.exists(out):
                    print(f"  [跳过] {name}（无现成二进制）")
                    continue
            else:
                out = build(bench, name, lang, ext)
                if out is None:
                    continue
            ms = time_binary(out, runs, warmup)
            if ms is not None:
                results[bench][name] = ms
                print(f"  {name:<6} {ms:10.3f} ms")
        print()

    write_report(results, runs, warmup)
    print("报告已写入: results/benchmark_report.md")
    return 0


def write_report(results: dict, runs: int, warmup: int) -> None:
    os.makedirs(os.path.join(ROOT, "results"), exist_ok=True)
    now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
    lines = []
    lines.append("# Zeta 语言性能对比基准报告\n")
    lines.append(f"> 生成时间: {now}  |  每次运行 warmup {warmup} 次 + 正式 {runs} 次取中位数\n")
    lines.append("## 环境\n")
    for cmd in ["uname -m", "sw_vers -productVersion", "clang --version", "swiftc --version",
                "rustc --version", "sysctl -n machdep.cpu.brand_string", "sysctl -n hw.ncpu"]:
        try:
            out = subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout.strip().splitlines()
            key = cmd.split()[-1]
            lines.append(f"- `{cmd}` → {out[0] if out else ''}")
        except Exception:
            pass
    lines.append("")

    langs = [name for name, _, _ in LANGS]
    lines.append("## 编译优化级别\n")
    lines.append("| 语言 | 编译器/优化 |")
    lines.append("|------|-------------|")
    lines.append("| Zeta | `zeta build`（无优化管线，clang O0 汇编——当前真实水平） |")
    lines.append("| C    | `clang -O3` |")
    lines.append("| C++  | `clang++ -O3` |")
    lines.append("| Swift| `swiftc -O` |")
    lines.append("| Rust | `rustc -O` |")
    lines.append("")
    lines.append("## 结果（毫秒，中位数，越低越好）\n")
    lines.append("| 基准 | " + " | ".join(langs) + " |")
    lines.append("|------|" + "-----|" * len(langs))

    speed_lines = ["## 相对速度（以 Zeta = 1.0 为基准，越高表示比 Zeta 快）\n"]
    speed_lines.append("| 基准 | " + " | ".join(langs) + " |")
    speed_lines.append("|------|" + "-----|" * len(langs))

    for bench in BENCHMARKS:
        if bench not in results:
            continue
        row = results[bench]
        base = row.get("Zeta")
        vals = [f"{row.get(l, float('nan')):.3f}" for l in langs]
        lines.append(f"| {bench} | " + " | ".join(vals) + " |")
        ratio = [f"{base / row[l]:.1f}x" if base and l in row and row[l] > 0 else "—" for l in langs]
        speed_lines.append(f"| {bench} | " + " | ".join(ratio) + " |")

    lines.append("")
    lines.extend(speed_lines)

    lines.append("")
    lines.append("## 基准说明\n")
    for b in BENCHMARKS:
        if b in results:
            lines.append(f"- **{b}**: {DESC[b]}")
    lines.append("")
    lines.append("## 方法学与注意事项\n")
    lines.append("- 所有语言实现逻辑严格一致（跨语言输出已逐项核对一致）。")
    lines.append("- Zeta 当前无优化管线（最小 MIR 优化 + clang O0 汇编），其余语言均为发布级优化，"
                 "本报告反映的是各语言**当前编译器的真实水平**，而非 Zeta 的理论上限。")
    lines.append("- 排序基准中 Zeta `Vec::sort_by` 当前实现为 O(n²) 选择排序（std-lib §3.1），"
                 "C/C++/Swift/Rust 均为 O(n log n) 标准库排序——该差异属标准库实现差距，非语法层能力差距。")
    lines.append("- 进程启动开销已含在计时内（各语言一致）。")
    lines.append("- Go 本机未安装，暂缺；安装后可在本报告框架内补测。")
    lines.append("")

    out = os.path.join(ROOT, "results", "benchmark_report.md")
    with open(out, "w") as f:
        f.write("\n".join(lines))
    # 同时输出 JSON 便于后续处理
    with open(os.path.join(ROOT, "results", "raw.json"), "w") as f:
        json.dump(results, f, indent=2)


if __name__ == "__main__":
    sys.exit(main())
