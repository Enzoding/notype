#!/bin/bash
set -euo pipefail

echo "=================================================="
echo "    Notype 网关性能与端到端延迟基准评测套件"
echo "=================================================="

# 1. 确保音频样例存在
if [ ! -f "tests/fixtures/audio/sample_short.wav" ]; then
    echo "未检测到基准音频样例，自动生成中..."
    bash scripts/generate-samples.sh
fi

echo "开始执行延迟基准测试与网络评测..."
cargo test --locked providers::live -- --ignored --nocapture --test-threads=1

echo "=================================================="
echo "    基准评测运行完毕！"
echo "=================================================="
