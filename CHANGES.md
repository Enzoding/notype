# 变更日志 (CHANGES)

本文档记录 Notype 应用的所有重要改动、功能迭代与缺陷修复。版本号遵循 [语义化版本规范 (SemVer)](https://semver.org/lang/zh-CN/)。

---

## [0.4.0] - 2026-10-26

### 新增功能 (Features)
- 应用内版本展示：侧边栏应用品牌区显示原生胶囊版本标（`v0.4.0`），设置 -「本机」模块新增「关于与更新」卡片展示当前版本详情。
- GitHub Releases 检查更新机制：
  - 新增 `updater` 独立模块，对接 GitHub Releases API 查询最新 Release 发布包。
  - 支持 SemVer 规范对比、网络代理自动适配、以及针对私有仓库的 `GITHUB_TOKEN` 授权兼容。
- 应用内增量下载与实时进度展示：
  - 发现新版本时展示版本标题、文件大小与发布说明。
  - 支持后台流式下载 DMG 安装包至本地更新缓存，实时显示下载进度百分比与已下载字节量。
- 原生一键重启并安装更新：
  - 下载完成后支持「重启并安装」，通过安全后台更新脚本自动挂载 DMG、替换 `.app` 应用程序包并自动拉起重启。
  - 包含容错回退机制：当 `/Applications` 存在权限限制时，自动降级为在 Finder 中挂载打开 DMG 便于用户拖拽安装。

### 工程治理与文档 (Engineering & Documentation)
- `locales/en.json` 与 `locales/zh-Hans.json` 新增 15 个双语词条，覆盖版本检查、下载进度、安装重启全链路状态。
- 新增 SemVer 解析与大小对比单元测试，保持纯内存渲染与零磁盘阻塞 UI 响应。

## [0.3.0] - 2026-10-25

### 新增功能 (Features)
- 完成多语言国际化里程碑（I0 ~ I4 全阶段闭环）：
  - macOS 权限提示多语言化（计划 I4）：`scripts/package.sh` 打包流程新增 `en.lproj` 与 `zh-Hans.lproj`（以及 `zh_CN.lproj`）本地化 `InfoPlist.strings`，系统级麦克风、语音识别、辅助功能权限弹窗随 macOS 系统语言本地化呈现。
  - `Info.plist` 显式声明 `CFBundleDevelopmentRegion = en` 及 `CFBundleLocalizations = [en, zh-Hans]`。

### 工程治理与文档 (Engineering & Documentation)
- `scripts/package.sh` 打包流程加入 `plutil -lint` 自动静态语法校验，保障 `.plist` 与 `.strings` 文件规范合规。
- 更新 `AGENTS.md` 规范与 `docs/iteration-plan-2026-10-25.md`，将中英文双语字典维护与无持久化翻译作为长期开发准则。

## [0.2.7] - 2026-10-25

### 新增功能 (Features)
- 强类型任务状态枚举 `TaskPhase`（计划 I3）：
  - 任务状态由原本写入持久化 JSON 的中文字符串重构为强类型枚举 `TaskPhase`（涵盖 `recording`, `saved`, `transcribing`, `transcribed`, `polishing`, `polish_retry_pending`, `ready_for_delivery`, `completed`, `cancelled`, `interrupted`, `audio_save_error`, `too_short`）。
  - 彻底解耦存储层内部状态与用户呈现文案，支持多语言动态展示（`TaskPhase::label()` 通过 `i18n::t` 动态查询当前系统或偏好语言）。

### 缺陷修复 (Fixes)
- 历史任务向前兼容反序列化：自定义 `Deserialize` 同时支持新版 Canonical ASCII 蛇形命名与历史版本中文状态（如 `"录音中"`、`"已完成"`、`"润色待重试"`），未知状态安全优雅降级为 `Interrupted`，彻底杜绝老旧草稿任务丢失。

### 工程治理与文档 (Engineering & Documentation)
- 新增 `TaskPhase` 序列化与向前兼容单元测试（覆盖全部 12 种历史中文状态及未知状态容错）。
- 确保 `tasks` 模块零 GPUI 依赖，遵循 `AGENTS.md` 边界约束。

## [0.2.6] - 2026-10-25

### 新增功能 (Features)
- 全界面多语言与中英文本地化支持（计划 I1 / I2）：
  - 核心框架与浮层：录音状态、麦克风权限状态、悬浮语音胶囊计时与状态、主窗口侧边栏状态卡与标签完成中英文本地化。
  - 界面工作台：初始化引导（Setup）、总览页面（Overview 统计卡与最近口述）、历史记录（History 搜索、操作、详情、分页与原文对比）、个人词库（Dictionary 词条添加、推荐预设、拼写纠正与分页）完成全面本地化。
  - 设置面板（Settings）：支持系统设置、录音与恢复、快捷键与麦克风、润色与学习、AI 渠道及模型全部分区的双语展示。
  - 运行中语言切换：设置 -「本机」新增「界面语言」选择器（跟随系统 / 简体中文 / English），切换即时生效并自动联动 macOS 状态栏托盘菜单与主界面重新渲染。
  - 领域枚举与常用应用名：`PolishMode`、`TriggerMode`、`AutoLearnMode`、`TermKind` 及 macOS 常见原生与第三方应用友好名称全面本地化。

### 工程治理与文档 (Engineering & Documentation)
- `locales/en.json` 与 `locales/zh-Hans.json` 扩充并严格对齐 287 个词条键与占位符，单元测试强制校验词条 100% 互相覆盖且在源码中完全引用。
- `i18n::t` 返回 `&'static str`，渲染路径保持纯内存哈希索引且零堆内存分配。

### 新增功能 (Features)
- 启用新版主应用视觉与图标（流动语音落笔 monogram）：生成并更新 1024×1024 高清图标、Logo 以及涵盖 16~1024 各级分辨率的 `AppIcon.icns`，采用 macOS 标准双层微阴影与超采样平滑圆角。
- 升级 `scripts/make_icon.py` 自动化图标生成脚本，统一 macOS 图标规范导出流程。

## [0.2.4] - 2026-10-25

### 工程治理与文档 (Engineering & Documentation)
- 新增 `providers::live` 手动冒烟测试（`#[ignore]`）：用环境变量传入 Key 和合成语音，对真实 Vercel AI Gateway 验证聊天连接、模型目录、转写、润色与术语提取；不打印 Key，默认不运行。

## [0.2.3] - 2026-10-25

### 新增功能 (Features)
- 多语言基础设施（计划 I0）：新增 `i18n` 模块、`ui_language` 设置（跟随系统 / 简体中文 / English，默认跟随系统）和中英文词条文件；启动时按 macOS 首选语言解析，中文系统保持原有界面，英文系统使用英文，繁体或其它语言回退英文。
- 状态栏菜单已使用词条，是首个本地化界面；其余界面按计划分步迁移。

### 工程治理与文档 (Engineering & Documentation)
- 新增语言解析、回退、词条键与占位符一致性、源码引用键与词条互相覆盖的测试；系统语言读取只在启动或切换偏好时执行。
- 语言选择器和运行中刷新尚未接入界面（`Session::set_ui_language` 已就绪）。

## [0.2.2] - 2026-10-25

### 缺陷修复 (Fixes)
- 清理全部 Clippy 警告（挖掘、润色、存储、历史分页、词典行组件、启动入口），行为不变；词典行的三个主题色合并为 `PhraseRowColors`。
- 包描述更新为 Vercel AI Gateway 或 OpenRouter。

### 工程治理与文档 (Engineering & Documentation)
- 新增 Vercel 转写请求线上契约测试：用本地 socket 校验协议头（`ai-gateway-protocol-version`、模型规格版本、模型 ID、Bearer）与 JSON 请求体，不产生真实请求。
- 新增 `#[ignore]` 基线测试 `baseline_ten_minutes_of_48khz_stereo`，度量十分钟 48 kHz 立体声的写盘、分片编码与内存。
- 迭代方案补充登录方式决定（邮箱验证码 + Resend，GitHub/Google 后续）与多语言（English / 简体中文）方案；`docs/p0-p1-validation.md` 追加 0.2.2 验证记录。

## [0.2.1] - 2026-10-25

### 新增功能 (Features)
- 自带密钥收口为 Vercel / OpenRouter 两个渠道，切换一次后转写、润色、术语学习统一使用当前渠道。
- 工作台首个模块只显示 API Key 与所需权限两项；缺少时直接引导配置，Key 与麦克风/辅助功能权限齐备后显示 OK。
- 新增原生麦克风权限查询和授权入口；权限状态后台定期刷新，渲染只读取缓存。

### 缺陷修复 (Fixes)
- 服务地址固定跟随渠道，移除地址编辑入口；旧自定义地址不再作为当前渠道地址，原数据和模型设置保留。
- 避免把检测到麦克风设备等同于已获得录音授权；模型测试不影响首页的配置完成状态。
- 旧的分阶段渠道选择沿用原转写渠道，保留另一渠道已保存的密钥和模型。

### 工程治理与文档 (Engineering & Documentation)
- 偏好设置分为五个主题，只挂载当前主题；主题导航固定，滚动区域显式约束最小高度。
- 模型目录按需展开，预先按模型类型建立索引，每类最多挂载六项；恢复任务正文只在复制时取值。
- 新增统一渠道迁移、固定地址、两项就绪判断、权限枚举及大模型目录挂载上限的回归测试。

## [0.2.0] - 2026-10-25

### 新增功能 (Features)
- 最长录音从两分钟扩展为十分钟；后台音频日志每两秒持久化检查点，转写按 30 秒分片保存，可从失败片继续处理。
- 偏好设置增加恢复任务列表、按当前配置重试、复制内容、删除草稿及分页；中断/失败草稿保留七天，恢复结果不会自动粘贴。
- 新增 OpenRouter 专用 STT 支持，Vercel/OpenRouter 分别保存配置；转写和润色/术语提取可选不同渠道。
- 支持填写模型 ID、刷新及缓存渠道模型目录，分别进行润色测试和合成语音转写测试。
- 新密钥保存在 macOS Keychain；旧 SQLite/.env.local 配置在钥匙串写入验证后迁移，保留无关配置。

### 缺陷修复 (Fixes)
- 修复转写失败丢失音频、麦克风断流只打印日志，以及录音结束时在 UI 线程编码整段音频的问题。
- 麦克风设备枚举移至后台，避免虚拟音频驱动阻塞应用启动；长稿分段润色并拒绝将截断结果当作成功。
- 保留润色失败回退原文，并支持只重试润色；恢复历史使用事务与任务映射防止重复记录。
- 修复含 /v1 的 Base URL 重复拼接，禁止携带凭据的地址和重定向，避免上游错误正文暴露用户输入。
- 429/502/503/504 有界退避，取消阻断后续请求和迟到交付；网络超时保留任务并提示可能重复计费。

### 工程治理与文档 (Engineering & Documentation)
- 拆分 providers、tasks、Keychain 和恢复 UI；连接状态收拢为 ConnectionState。
- 增加十分钟合成音频、检查点、取消、重试、协议格式、历史幂等及旧 schema 回归测试。
- 新增 NOTYPE_DATA_DIR 隔离开发数据与开发钥匙串，更新 AGENTS.md 和恢复格式说明；统一现有 Rust 文件的 rustfmt 格式。

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
