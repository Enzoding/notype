#!/bin/bash
set -euo pipefail
DIR="tests/fixtures/audio"
mkdir -p "$DIR"

echo "Generating synthetic voice samples with macOS say + afconvert..."

# 1. Short utterance (~2s)
say -v "Ting-Ting" -o "$DIR/temp_short.aiff" "今天下午两点开会。"
afconvert -f WAVE -d LEI16@16000 "$DIR/temp_short.aiff" "$DIR/sample_short.wav"
rm "$DIR/temp_short.aiff"

# 2. Technical mixed utterance (~5s)
say -v "Ting-Ting" -o "$DIR/temp_tech.aiff" "请帮我把这个 PR merge 到 main 分支，然后跑一下 CI 流程。"
afconvert -f WAVE -d LEI16@16000 "$DIR/temp_tech.aiff" "$DIR/sample_technical.wav"
rm "$DIR/temp_tech.aiff"

# 3. Disfluent oral utterance (~4s)
say -v "Ting-Ting" -o "$DIR/temp_disfluent.aiff" "呃，我想说这个这个事情，我们明天去上海开会。"
afconvert -f WAVE -d LEI16@16000 "$DIR/temp_disfluent.aiff" "$DIR/sample_disfluent.wav"
rm "$DIR/temp_disfluent.aiff"

echo "Generated:"
ls -lh "$DIR"/*.wav
