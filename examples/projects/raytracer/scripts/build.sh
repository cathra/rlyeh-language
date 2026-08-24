#!/usr/bin/env bash
# P1 raytracer：编译为可执行文件
set -e
cd "$(dirname "$0")/.."
exec ../../../target/release/zeta-driver build src/main.zeta -o raytracer --force
