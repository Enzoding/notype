# 变更日志 (CHANGES)

本文档记录 Notype 应用的所有重要改动、功能迭代与缺陷修复。版本号遵循 [语义化版本规范 (SemVer)](https://semver.org/lang/zh-CN/)。

---

## [0.1.1] - 2026-10-25

### 新增功能 (Features)
- **开源版 API Key 与接口自主配置**：
  - 将 API Key 与 Base URL 的主存储迁移至本地 SQLite 数据库（`settings` 表），用户在应用内输入即可直接持久化，彻底摆脱必须手动创建或修改 `.env.local` 文件的限制。
  - 支持自定义 API 网关 Base URL（默认预设 `https://ai-gateway.vercel.sh`，可自由切换至兼容反向代理、Cloudflare AI Gateway 或 OpenAI 格式服务）。
  - 支持常用语音转写模型（`mai-transcribe-2`、`whisper-large-v3`）与智能润色模型（`deepseek-v4.1-flash`、`gpt-4o-mini`、`claude-3-5-haiku`）的直观切换。
  - 首屏概览（Overview）增加未配置 Key 时的引导卡片，直达连接配置页。
- **macOS 系统代理自动侦测与适配**：
  - 针对国内网络环境下访问国外 AI 网关超时的痛点，以及 macOS GUI 应用从 Finder 启动不继承终端 shell 环境变量（`http_proxy`/`all_proxy`）的问题，在网络客户端中自动读取 `scutil --proxy` 获取系统代理并注入 `reqwest`，完美适配 Clash / Surge / Shadowrocket 等代理工具。
  - 设置页面提供一键「测试连接」按钮，实时测量往返延迟（ms）并展示错误诊断详情，显示当前系统代理适配状态。

### 缺陷修复 (Fixes)
- **全局快捷键与 Esc 终止无响应修复**：
  - 修复未授权辅助功能权限时快捷键监听线程直接退出的缺陷；引入 `AXIsProcessTrustedWithOptions` 主动触发系统授权弹窗，并增加 800ms 轮询重试机制，用户在系统设置授权后无需重启应用即可自动生效。
  - 统一 `paste.rs` 与 `hotkey.rs` 中 `CFRelease` 的 FFI 外部链接声明签名，消除符号冲突隐患。
- **网络连接超时处理**：
  - 为 `reqwest` 增加 `rustls-tls-native-roots` 与 `socks` 特性支持，修复直接直连导致 5 秒超时的网络请求失败问题。

### 规范与工程治理 (Engineering & Documentation)
- **自动化版本注入**：重构 `scripts/package.sh`，自动从 `Cargo.toml` 提取版本号同步至 `Info.plist` 与 DMG 文件名。
- **项目规范沉淀**：
  - 建立 `CHANGES.md` 追踪所有版本改动。
  - 建立 `LESSONS.md` 记录深层排查经验、常见易错点与解决方案。
  - 在 `AGENTS.md` 中增加版本号迭代要求、变更记录要求与踩坑经验记录规范。

---

## [0.1.0] - 2026-10-25

### 新增功能 (Features)
- **品牌名称更新**：正式更名为 **Notype**，更新全套 Cargo 清单、窗口标题、托盘菜单与界面文案。
- **人文编辑风格图标**：设计并生成全新的纸质墨水笔触感 macOS 应用图标（包含 1024x1024 PNG 与全套多尺寸 ICNS）。
- **打包与安装脚本**：提供一键打包脚本 `scripts/package.sh`，直接在 `release/` 目录下生成 `Notype.app` 与 `Notype-0.1.0.dmg` 安装包。
- **存储向后兼容**：自动平滑兼容旧版本 `OpenTypeless` 本地数据库与音频目录。
