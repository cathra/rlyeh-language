#!/usr/bin/env bash
# P1 raytracer：基准测试（zeta bench 对渲染流水线计时）
set -e
cd "$(dirname "$0")/.."
exec ../../../target/release/zeta-driver bench src/main.zeta --force
