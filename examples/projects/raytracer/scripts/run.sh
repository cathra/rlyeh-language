#!/usr/bin/env bash
# P1 raytracer：编译并运行，生成 output.ppm
set -e
cd "$(dirname "$0")/.."
../../../target/release/rlyeh-driver run src/main.rl --force
# macOS 预览：转 PNG 并用系统应用打开
if command -v sips >/dev/null 2>&1; then
    sips -s format png output.ppm --out output.png >/dev/null 2>&1
    echo "preview: output.png"
fi
