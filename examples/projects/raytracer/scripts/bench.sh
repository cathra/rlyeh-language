#!/usr/bin/env bash
# P1 raytracer：基准测试（rlyeh bench 对渲染流水线计时）
set -e
cd "$(dirname "$0")/.."
exec ../../../target/release/rlyeh-driver bench src/main.rl --force
