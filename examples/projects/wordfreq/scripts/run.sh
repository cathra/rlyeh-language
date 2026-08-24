#!/bin/sh
# 运行 wordfreq（需在项目目录：读取 sample.txt，输出排行并写 freq.txt）
cd "$(dirname "$0")/.."
exec ./wordfreq
