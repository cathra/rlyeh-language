#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""从 Rust 源文件中拆出一个 impl 块内的方法到多个子模块文件。

用法:
    python3 scripts/split_impl.py <源文件> <impl类型名> <方法起始行(1-based)> <方法结束行(1-based)> '<JSON配置>'
JSON配置: {"子模块名": [方法名, ...], ...}

- 每个方法从源 impl 块剪切到对应子模块的 `impl <类型> { ... }` 块
- 源文件保留其余方法，末尾追加 mod 声明
"""
import re
import sys
import os
import json


def find_method_end(lines, start):
    """从方法定义行 start 开始，词法级花括号配对，返回方法体结束行索引（含）。"""
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
            if c == '"' or c == "'":
                in_str = c
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
    raise RuntimeError(f"方法起始 {start} 未找到结束")


def main():
    src = sys.argv[1]
    impl_ty = sys.argv[2]
    start_line = int(sys.argv[3]) - 1  # 0-based
    end_line = int(sys.argv[4]) - 1
    config = json.loads(sys.argv[5])

    with open(src, "r", encoding="utf-8") as f:
        content = f.read()
    lines = content.split("\n")

    # 在 impl 块内找方法
    # 方法定义：缩进4空格 + fn name(
    method_re = re.compile(r"^    fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")
    methods = {}
    i = start_line
    while i <= end_line:
        line = lines[i]
        m = method_re.match(line)
        if m:
            name = m.group(1)
            end = find_method_end(lines, i)
            # 方法体从 i 到 end
            text = "\n".join(lines[i:end + 1])
            methods[name] = (i, end, text)
            i = end + 1
        else:
            i += 1

    # 迁移方法
    migrated = set()
    module_files = {}
    for mod, names in config.items():
        chunks = []
        for n in names:
            if n in methods:
                chunks.append(methods[n][2])
                migrated.add(n)
            else:
                print(f"[WARN] 方法 {n} 未找到")
        if chunks:
            header = f"""//! {mod}：LLVM 发射子模块。
//! （由 {os.path.basename(src)} 的 `impl {impl_ty}` 拆分而来，保持语义等价）

use super::*;

impl {impl_ty} {{
"""
            body = "\n\n".join(chunks)
            footer = "}\n"
            module_files[mod] = header + body + footer

    # 重建主文件：删除已迁移的方法行（从 impl 块中）
    remove = set()
    for n in migrated:
        remove.update(range(methods[n][0], methods[n][1] + 1))
    kept = [l for i, l in enumerate(lines) if i not in remove]

    # 写入子模块文件（同目录）
    out_dir = os.path.dirname(src)
    for mod, text in module_files.items():
        path = os.path.join(out_dir, f"{mod}.rs")
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)
        print(f"写 {path}: {len(text.splitlines())} 行")

    # 主文件末尾追加 mod 声明 + use
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
