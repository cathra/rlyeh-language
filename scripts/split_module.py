#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""通用模块拆分脚本：把一个 Rust 文件按函数名拆分为多个子模块文件。

用于 check_expr 子模块的二次拆分。用法：
    python3 scripts/split_module.py <源文件> '<JSON 配置>'
JSON 配置: {"目标子模块名": [函数名, ...], ...}
- 每个函数从源文件剪切到对应子模块文件（新建，带 use super::* 头）
- 源文件保留其余函数 + mod 声明 + use，默认不保留任何 API（调用方处理）
"""
import re
import sys
import os
import json


def find_func_end(lines, start):
    """词法级花括号配对，找到函数体右花括号所在行索引。"""
    depth = 0
    in_str = None
    i = start
    brace_started = False
    while i < len(lines):
        line = lines[i]
        j = 0
        while j < len(line):
            c = line[j]
            if in_str:
                if c == '\\':
                    j += 2
                    continue
                if c == in_str:
                    in_str = None
                j += 1
                continue
            if c == '"':
                in_str = '"'
                j += 1
                continue
            if c == "'":
                # 区分字符字面量 'x' 与生命周期 'static：
                # 只有 '后跟(转义)单字符再跟' 才视为字符字面量
                # 向前找下一个单引号
                rest = line[j + 1:]
                nq = rest.find("'")
                if nq != -1 and nq <= 3:  # 'a' 或 '\n'
                    in_str = "'"
                    j += 1
                    continue
                # 否则是生命周期或普通用法，忽略
                j += 1
                continue
            if c == '/':
                if j + 1 < len(line):
                    nxt = line[j + 1]
                    if nxt == '/':
                        break
                    elif nxt == '*':
                        k = line.find('*/', j + 2)
                        if k != -1:
                            j = k + 2
                            continue
                        else:
                            i += 1
                            while i < len(lines):
                                if '*/' in lines[i]:
                                    break
                                i += 1
                            j = len(line)
                            continue
            if c == '{':
                depth += 1
                brace_started = True
            elif c == '}':
                depth -= 1
                if brace_started and depth == 0:
                    return i
            j += 1
        i += 1
    raise RuntimeError(f"函数起始 {start} 未找到结束")


def main():
    src = sys.argv[1]
    config = json.loads(sys.argv[2])
    with open(src, "r", encoding="utf-8") as f:
        content = f.read()
    lines = content.split("\n")

    # 兼容 pub(super) / pub(crate) / pub / 普通 fn
    fn_re = re.compile(r"^(?:(pub|pub\(crate\)|pub\(super\))\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")
    funcs = {}
    for idx, line in enumerate(lines):
        m = fn_re.match(line)
        if m:
            vis = m.group(1) or "fn"
            name = m.group(2)
            end = find_func_end(lines, idx)
            # 向上回溯包含紧邻的属性行（#[test] / #[cfg(...)]）
            start = idx
            k = idx - 1
            while k >= 0 and lines[k].strip().startswith('#'):
                start = k
                k -= 1
            text = "\n".join(lines[start:end + 1])
            funcs[name] = (start, end, text, vis)

    # 迁移函数
    migrated = set()
    module_files = {}
    for mod, fns in config.items():
        chunks = []
        for fn in fns:
            if fn in funcs:
                chunks.append(funcs[fn][2])
                migrated.add(fn)
            else:
                print(f"[WARN] {fn} 未找到")
        if chunks:
            header = f"""//! 表达式检查子模块：{mod}。
//! （由 {os.path.basename(src)} 二次拆分而来，保持语义等价）

use super::*;

"""
            module_files[mod] = header + "\n\n".join(chunks) + "\n"

    # 源文件保留未迁移的函数
    keep_idx = set()
    for idx, (name, _end, _text, _vis) in funcs.items():
        pass
    # 重建保留行
    remove_ranges = set()
    for fn in migrated:
        remove_ranges.update(range(funcs[fn][0], funcs[fn][1] + 1))
    kept = [l for i, l in enumerate(lines) if i not in remove_ranges]

    # 写子模块文件（放在源文件同目录）
    out_dir = os.path.dirname(src)
    for mod, content_text in module_files.items():
        path = os.path.join(out_dir, f"{mod}.rs")
        with open(path, "w", encoding="utf-8") as f:
            f.write(content_text)
        print(f"写 {path}: {len(content_text.splitlines())} 行")

    # 在源文件末尾追加 mod 声明 + use
    final = kept[:]
    final.append("")
    for mod in module_files:
        final.append(f"mod {mod};")
    final.append("")
    for mod in module_files:
        final.append(f"use {mod}::*;")
    out = "\n".join(final)
    out = re.sub(r"\n{4,}", "\n\n\n", out)
    with open(src, "w", encoding="utf-8") as f:
        f.write(out + "\n")
    print(f"源文件剩余 {len(final)} 行")


if __name__ == "__main__":
    main()
