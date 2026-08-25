#!/usr/bin/env bash
# P1 raytracer：编译为可执行文件
set -e
cd "$(dirname "$0")/.."
exec ../../../target/release/rlyeh-driver build src/main.rl -o raytracer --force
