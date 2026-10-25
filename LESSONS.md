# 经验与故障排查知识库 (LESSONS)

本文档记录在 Notype 开发与维护过程中遇到的深层疑难杂症、失败教训与易错点，用于警示后续开发，防止重复踩坑。

---

## 1. macOS GUI 应用环境变量缺失与系统代理机制

- **问题现象**：
  在终端中运行 `cargo run` 或执行 `curl` 请求 AI 网关非常流畅，但打包成 macOS `.app` 安装包并在 Finder 中双击启动后，所有的网络请求均失败（报超时 5s 或网络客户端错误）。
- **根本原因**：
  macOS 桌面环境下通过 LaunchServices（Finder 双击、Spotlight、Dock）启动的应用程序，运行在系统的干净守护进程环境中，**不会**加载用户的 shell 配置文件（如 `~/.zshrc` 或 `~/.bash_profile`）。因此终端中配置的 `http_proxy`、`https_proxy`、`all_proxy` 全都不存在。在中国大陆网络环境下，海外 AI 网关（如 `ai-gateway.vercel.sh`）直接连接会超时，必须依赖本地系统代理（如 Clash / Surge 监听的 `127.0.0.1:7897`）。
- **防范与解决方案**：
  1. 绝不假设 GUI 应用拥有终端环境变量。
  2. 在网络初始化前，执行 `scutil --proxy` 获取 macOS 系统级活跃代理设置（解析 `HTTPSEnable`、`HTTPPort`、`HTTPSProxy`、`SOCKSEnable` 等）。
  3. 当检测到环境变量为空且系统代理开启时，动态将系统代理注入到 `reqwest::blocking::ClientBuilder::proxy`。
  4. 确保 `Cargo.toml` 中 `reqwest` 开启了 `socks` 与 `rustls-tls-native-roots` 特性。

---

## 2. macOS 辅助功能权限 (Accessibility) 与监听线程生命周期

- **问题现象**：
  全新安装应用后，按下全局快捷键（如 Option+Space）无任何响应，录音过程中按下 Esc 也无法终止或取消。
- **根本原因**：
  macOS 的全局键盘事件监听基于 `CGEventTapCreate`，必须获得系统的「辅助功能」授权。旧代码在未授权时尝试创建事件源失败，直接 `return` 退出了后台监听线程。这导致即使用户随后在「系统设置 → 隐私与安全性 → 辅助功能」中勾选了应用权限，后台监听线程也已经死亡，无法恢复，必须强制退出并重启应用。
- **防范与解决方案**：
  1. 应用启动与监听器初始化时，调用 `AXIsProcessTrustedWithOptions`（传入 `kAXTrustedCheckOptionPrompt = true`），主动唤起系统的辅助功能授权对话框。
  2. 监听线程在创建 Tap 失败时不应直接终止，而应在后台保持轻量重试循环（例如每 800ms 检查一次权限），一旦用户在系统偏好中授予权限，监听线程立刻自动激活，无需用户重启应用。

---

## 3. 开源产品思维 vs 开发者本地模式思维

- **问题现象**：
  早期实现将 API Key 读取绑定在代码根目录或 Application Support 目录下的 `.env.local` 文件，且硬编码了特定网关地址。对于最终用户而言，根本不知道什么是 `.env.local`，也无法方便地切换兼容反代、自建网关或私有模型。
- **根本原因**：
  混淆了「开发者本地方便」与「开源终端用户产品体验」。开源桌面应用应当是自包含的、开箱即用的，不能强制要求用户使用终端编写配置文件。
- **防范与解决方案**：
  1. 用户配置（API Key、Base URL、模型选择）一律以本地 SQLite 数据库（`settings` 表）为主存储，在 UI 界面中直接提供可视化输入与修改入口。
  2. 提供 Base URL 自定义输入，默认预设官方服务，支持一键重置为默认值。
  3. 提供显式的「测试连接」按钮，当场发起测试并反馈延迟（ms）或清晰的错误原因（如 401 密钥失效、网络连接超时等），杜绝黑盒故障。
  4. 未配置 Key 时在首屏显式给出引导提示，避免用户盲目触发快捷键时遇到未知报错。

---

## 4. GPUI 高频渲染纯度与事件监听风暴

- **问题现象**：
  历史记录列表滚动时卡顿掉帧，或悬停在列表项上方时界面出现细微延迟。
- **根本原因**：
  1. GPUI 的刷新频率高达 60Hz~120Hz，每次滚动 offset 变化都会触发 `Render::render`。如果在 render 阶段调用任何同步磁盘 I/O（如 `std::path::Path::exists`、`is_file`），会严重阻塞主线程渲染管道。
  2. 在列表行渲染中，如果使用 `group_hover` 但每行使用的是相同的静态 group 名称（如 `group_hover("hover-row", ...)`），GPUI 会为所有行挂载相同的选择器监听，引发鼠标监听器风暴与 Hitbox 计算碰撞。
- **防范与解决方案**：
  1. `Render::render` 内部绝对保持纯内存运算。录音文件的存在性等状态必须预先在 `Session` 中维护（如 `audio_ids: HashSet<i64>`），渲染时仅做内存查询。
  2. 列表循环中的 `group` 必须保证每一行拥有独立的 group 标识（如 `SharedString::from(format!("history-row-{id}"))`）。
  3. 对历史记录与词库等长列表实施分页或初始条数截断，防止一次性挂载成百上千个 Flexbox 布局节点。

---

## 5. FFI C 外部符号跨模块重复声明冲突

- **问题现象**：
  编译过程中出现外部符号类型不兼容或链接警告（如 `CFRelease` 符号重复且形参指针可变性不一致）。
- **根本原因**：
  在 `paste.rs` 中声明了 `fn CFRelease(cf: *const c_void)`，而在 `hotkey.rs` 中声明了 `fn CFRelease(cf: *mut c_void)`，两者链接到同一个 CoreFoundation 符号时引发签名冲突。
- **防范与解决方案**：
  1. 底层平台 FFI 尽量集中在专门的平台模块，或统一使用兼容的指针签名（`*mut std::ffi::c_void`）。
  2. 声明前检查项目中已有的 extern 声明，避免重复且互斥的同名外部函数引入。
