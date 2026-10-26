# 开发构建磁盘维护

默认开发和测试构建使用 `debug = "line-tables-only"`、`incremental = false`。
保留堆栈文件与行号，但不保留完整变量调试信息；修改后编译可能更慢。
发布构建原有 LTO、体积优化和 strip 设置保持不变。

## 日常命令

```sh
# 查看当前 checkout 的 target，不修改文件
bash scripts/build-cache.sh report
# 4 GiB 维护预算；超限返回 1，参数错误返回 2
bash scripts/build-cache.sh check
# 可按机器调整预算，例如 6 GiB
NOTYPE_BUILD_BUDGET_MIB=6144 bash scripts/build-cache.sh check
# 停止编译后，按需清理开发产物或全部构建产物
bash scripts/build-cache.sh clean-dev
bash scripts/build-cache.sh clean-all
```

脚本只管理仓库根目录的 `target/`，不跟随该目录的符号链接。
不会删除 `release/` 安装包、共享 `~/.cargo` 缓存、Rust 工具链或应用用户数据。
`CARGO_TARGET_DIR` 指向的其他目录不纳入统计，需单独维护。
清理后下次编译会重新生成产物；不要在编译期间执行清理。

## 持续控制

只需检查类型时优先 `cargo check --locked`，需要可执行文件时再 build/run。
功能验证仍应正常运行测试和 Clippy。避免无必要地切换 profile、编译参数、
工具链或目标架构，它们可能生成多套缓存。配置改变不会自动清除旧产物。

需要 LLDB 查看变量时可临时使用 `CARGO_PROFILE_DEV_DEBUG=2 cargo build --locked`，
调试结束后清理并恢复默认构建。不要长期提交完整调试信息配置。

预算不是硬配额，也没有后台自动删除。依赖变化或完成构建验证后运行 check；
超限先 report 定位，再显式清理不需要的 profile。不要在每次构建前清理，
否则会失去依赖复用并增加编译成本。新的预算应依据实际测量调整。

## 初始测量（2026-10-26）

优化前 `target/` 约 11 GiB：debug 约 9.5 GiB（deps 约 6.1 GiB、
incremental 约 3 GiB），release 约 1.7 GiB。资源与源码不足 10 MiB。
这些是本机当时的占用，不代表新构建的固定大小。

清理后使用新配置依次完成 `cargo test --locked`、`cargo build --locked`、
`cargo clippy --all-targets --locked`：target 实测 3311 MiB（约 3.2 GiB），
整个项目约 3.3 GiB，低于 4 GiB 维护预算。测试 72 通过、3 忽略，Clippy 无警告。
此测量未重新生成 release 缓存；同口径开发缓存从约 9.5 GiB 降至约 3.2 GiB。
后续打包会额外生成 release 产物，超预算时可在打包完成后清理不需要的构建缓存。
