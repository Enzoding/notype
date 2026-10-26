#!/bin/bash
# Operate only on this checkout's disposable Cargo artifacts.
set -euo pipefail
repo_dir="$(cd "$(dirname "$0")/.." && pwd)"
cache_dir="$repo_dir/target"
action="${1:-report}"

usage() {
    echo "Usage: bash scripts/build-cache.sh [report|check|clean-dev|clean-all]"
}

if [[ $# -gt 1 ]]; then usage >&2; exit 2; fi
# Never follow a target symlink into an unrelated cache.
if [[ -L "$cache_dir" ]]; then
    echo "Refusing symlinked target directory: $cache_dir" >&2
    exit 2
fi

case "$action" in
    report|check)
        if [[ -d "$cache_dir" ]]; then
            du -h -d 1 "$cache_dir"
            size_kib=$(du -sk "$cache_dir" | awk '{print $1}')
        else
            echo "No local target cache."
            size_kib=0
        fi
        if [[ "$action" == check ]]; then
            budget_mib="${NOTYPE_BUILD_BUDGET_MIB:-4096}"
            if ! [[ "$budget_mib" =~ ^[1-9][0-9]{0,6}$ ]]; then
                echo "NOTYPE_BUILD_BUDGET_MIB must be a positive integer (up to 7 digits)." >&2
                exit 2
            fi
            echo "Local target: $((size_kib / 1024)) MiB; budget: $budget_mib MiB"
            if (( size_kib > budget_mib * 1024 )); then
                echo "Over budget. Stop builds, inspect report, then explicitly clean-dev or clean-all." >&2
                exit 1
            fi
        fi
        ;;
    clean-dev)
        cargo clean --manifest-path "$repo_dir/Cargo.toml" --target-dir "$cache_dir" --profile dev
        ;;
    clean-all)
        cargo clean --manifest-path "$repo_dir/Cargo.toml" --target-dir "$cache_dir"
        ;;
    *) usage >&2; exit 2 ;;
esac
