# 多语言实施交接（English / 简体中文）

日期：2026-10-25。面向接手实施的 Agent。设计依据见 [iteration-plan-2026-10-25.md](iteration-plan-2026-10-25.md) 第 10 节；本文给出可直接执行的步骤、文件范围和验收。读之前先读仓库根的 `AGENTS.md` 和 `LESSONS.md`，并遵守其中全部规则（版本号、`CHANGES.md`、渲染纯度、模块边界）。

## 1. 目标

- 界面语言支持 `en` 与 `zh-Hans`，默认跟随 macOS 系统语言，用户可在设置里切换，切换立即生效。
- 中文系统上的现有界面文案保持不变；英文系统显示英文；繁体及其它语言回退英文。
- 不翻译：应用名 Notype、渠道/模型名（Vercel、OpenRouter）、快捷键名、算法数据（`mining.rs` 的停用词、预设词）。
- 界面语言与口述语言无关：不传给 ASR，不改动润色提示词（它已是英文且要求与转写同语言）。

## 2. 当前状态（0.2.4）

I0 已完成，可直接使用：

| 内容 | 位置 |
| --- | --- |
| 语言/偏好类型、解析规则（纯函数，带测试） | `src/i18n/lang.rs` |
| 读取系统首选语言（`NSLocale.preferredLanguages`） | `src/i18n/detect.rs` |
| 词条加载与查找：`t(key)`、`t_args(key, &[(name, value)])`、`set_language` | `src/i18n/catalog.rs` |
| 入口：`i18n::apply(Preference)`（读系统并设为当前） | `src/i18n/mod.rs` |
| 词条文件，扁平 JSON | `locales/en.json`、`locales/zh-Hans.json` |
| 设置项 `ui_language`（`system`/`zh-Hans`/`en`，缺省为 `system`） | `src/store.rs`：`ui_language()`、`set_ui_language()` |
| 启动时读取并应用；`Session.ui_language` 字段 | `src/session/mod.rs::Session::new` |
| 运行中切换（已写好，**尚无界面调用**） | `src/session/settings.rs::set_ui_language` |
| 首个接入点：状态栏菜单 4 项（键 `tray.*`） | `src/main.rs::install_tray` |

测试已强制：两份词条的键、占位符一致；源码里 `t("…")`/`t_args("…")` 引用的键必须存在；已定义的键必须被引用（否则测试失败）。因此**不要预先添加没有使用的词条**，每步只加本步用到的键。

注意：`t` 的返回值生命周期跟随 `key`，传字面量时是 `&'static str`，所以现有 `fn label(self) -> &'static str` 一类函数可以只把字符串字面量换成 `t("…")` 而不改签名。键必须是**字符串字面量**（测试靠源码扫描检查）；枚举到键的映射写成 `match` 里逐个 `t("…")`，不要拼接键名。

## 3. 词条约定

- 键用小写点分命名：`<领域>.<区域>.<含义>`，如 `overview.setup.key_missing`。领域沿用代码结构：`tray`、`shell`、`overview`、`setup`、`overlay`、`stage`、`notice`、`time`、`settings`、`providers`、`history`、`dictionary`、`mining`、`polish`、`permission`、`error`。不要建 `common`/`misc`。
- 中文词条逐字沿用现有文案（保证中文界面不变）；英文用自然、简洁的 macOS 风格：首字母大写的标题用 Title Case（如 `Open Workspace`），句子用句首大写，不带多余标点。
- 参数用 `{name}` 占位，两种语言占位符集合必须相同。数量相关文案用显式键（`time.minutes_ago` 一类，英文需区分 1 与多个时用 `.one`/`.other` 两个键），不要引入复数引擎。
- 禁止用翻译后的文字做逻辑判断或持久化（见第 5 步 I3 的反例）。
- 新增文案一律两份同时添加。

## 4. 步骤 I1：框架界面

版本 0.2.5（PATCH），在 `CHANGES.md` 记录。

范围（先 `grep` 每个文件的中文行，全部迁移；下列为主要位置）：

- `src/views/components/shell.rs`、`src/views/mod.rs`：侧栏导航、窗口主体、状态胶囊 `status_pill`。
- `src/views/overview.rs`、`src/views/components/overview.rs`、`src/views/setup.rs`：工作台首页、配置状态（API Key / 权限两项）、最近口述。注意 AGENTS.md 的限制：首个模块只显示这两项，不要借迁移增加内容。
- `src/overlay.rs`：浮动胶囊（“处理中”、“剩余 {}” 等）。
- `src/session/types.rs`：`Stage` 的两组标签、`Notice` 文案。
- `src/permissions.rs`：权限状态标签（已授权/待授权…）。
- `src/session/time.rs`：`format_relative_time`（刚刚、早期记录、N 分钟/小时/天前）。其测试断言中文字符串，需要改成在固定语言下断言（测试里先 `i18n::set_language(...)`，注意测试并行时语言是全局的——为避免互相干扰，把格式化拆成接受 `Language` 的内部函数再测试，或测试内串行化）。
- `src/main.rs`：状态栏菜单刷新（见下），以及窗口标题是否需要本地化（“Notype”、“Notype Voice” 是产品名，不翻译）。

状态栏菜单刷新：`install_tray` 现在创建后 `mem::forget(tray)`，菜单项无法再改文字。需要保留四个 `MenuItem` 的句柄（放入一个小结构，由 `main` 的协调循环持有），在语言变化时调用 `set_text`。建议在 `Session` 增加 `language_epoch: u64`，`set_ui_language` 自增，协调循环发现变化后刷新托盘。不要为此把整个 `Session` 传给托盘模块。

I1 不添加语言选择器，因此在 I2 之前只能通过隔离数据目录手动验证（见第 8 节）。

完成标准：切换语言（手动改库）后，上述界面全部为目标语言；中文界面与迁移前逐字一致；没有遗留的中文字面量（`grep` 复核，注释和数据除外）。

## 5. 步骤 I2：功能页面与语言选择器

版本 0.2.6。

- `src/views/settings.rs`、`src/views/components/settings.rs`、`src/views/providers.rs`、`src/views/history.rs`、`src/views/components/history.rs`、`src/views/dictionary.rs`、`src/views/components/dictionary.rs`、`src/session/settings.rs`、`src/session/providers.rs`、`src/session/studio.rs`、`src/session/mining.rs`：迁移全部可见文案。
- 枚举显示名：`src/polish.rs` 的 `PolishMode::label/short_label/description`、`friendly_app_name`（“当前应用/演练台/终端/微信/飞书…”；Xcode、Cursor 等品牌名不翻译）；`src/store.rs` 的 `TriggerMode` 标签；`src/mining.rs` 的 `AutoLearnMode`、来源、`TermKind` 等标签（约 20 行）。**`mining.rs` 里的停用词、预设词等数据不要动。**
- 注意 `src/session/mining.rs::persist_and_refresh(learning, label, …)` 的 `label`（如 “润色修正”）会拼进状态文案，用 `t_args` 并把标签也走词条。
- **语言选择器**：偏好设置新增“语言 / Language”主题或条目：跟随系统、简体中文、English。三个选项自身固定显示各语言原名（“简体中文”、“English”），“跟随系统”显示为当前语言下的文案。点击调用 `Session::set_ui_language`，随后移除该函数上的 `#[allow(dead_code)]`。设置页加一句说明：系统权限弹窗跟随 macOS 系统语言，不随此设置变化。
- 遵守 UI 规则：渲染里只调用 `t()`（内存查找），不得查询系统语言；设置主题仍只挂载当前主题；历史/词典分页和独立 group id 规则不变。
- 英文通常更长：检查固定宽度控件、侧栏、按钮、胶囊、表格列，不应溢出、截断或换行错位。

完成标准：在设置里切换语言后所有页面立即变化，无需重启；重启后保持所选语言；选“跟随系统”时按系统语言解析。

## 6. 步骤 I3：错误与持久状态

版本 0.2.7。这是风险最高的一步，先读 `src/tasks/` 并保护已有用户数据。

**问题**：`tasks::model::Task.phase`/`error` 是本地化后的中文字符串，且被写入 `task.json`，**代码还对它们做字符串比较**（`src/tasks/repository.rs` 约 199–218 行，如 `task.phase == "已完成"`、`matches!(… "录音中" | "转写中" | "润色中")`；`src/tasks/pipeline.rs` 多处赋值与测试断言；`src/tasks/journal.rs`；`src/views/providers.rs` 显示 `task.phase`）。切换语言后旧任务不会变语言，而且一旦翻译这些字符串，逻辑就会失效。

要求：

1. 新增 `Phase` 枚举（建议放 `src/tasks/model.rs` 或同目录新文件）：`Recording`、`Saved`、`Transcribing`、`Transcribed`、`Polishing`、`PolishRetryPending`、`ReadyToDeliver`、`Done`、`Cancelled`、`Interrupted`、`Corrupt` 等，覆盖现有全部取值。序列化为稳定的英文标识（`serde(rename_all = "snake_case")`）。
2. **向后兼容读取旧 `task.json`**：旧文件里的中文值（“录音中”“已保存”“转写中”“已转写”“润色中”“润色待重试”“待交付”“已完成”“已取消”“已中断，可恢复”“任务元数据损坏”等）需要映射到枚举；无法识别的值映射为 `Interrupted` 或 `Corrupt` 并保留原文件，**不得删除或覆盖用户任务**。用 `serde` 自定义反序列化或读取时迁移，二选一，需有测试覆盖每个旧值。
3. `error` 同理：存错误种类（枚举/错误码），显示时再翻译。`tasks/*`、`providers/*`、`capture.rs`、`session/*` 中大量 `map_err(|_| "中文…")` 返回展示文本，改为返回错误种类，由会话层或界面层翻译。可分阶段做，但每一处改动都要保证 `grep` 不再出现“用中文字符串比较或持久化”的情况。**错误提示和日志不得包含 API Key、音频、口述正文。**
4. `Summary`（恢复列表用）也改为携带枚举，视图里翻译。
5. 更新 `pipeline.rs`、`repository.rs` 等已有测试，把断言由中文字符串改为枚举；新增“读取旧中文 `task.json`”“未知 phase”“新写入的 json 为英文标识”的测试。
6. `AGENTS.md` 的“Recording Recovery”一节如涉及格式，补充说明 `phase` 的存储格式及旧版兼容。

完成标准：旧版本产生的任务目录可被新版本读取并按当前语言显示；新写入任务不含本地化文本；`grep -rn` 确认没有对翻译文本的比较。

## 7. 步骤 I4：打包与文档

版本 0.3.0（MINOR，多语言功能完整）。

- `scripts/package.sh`：为 `.app` 增加 `en.lproj/InfoPlist.strings` 与 `zh-Hans.lproj/InfoPlist.strings`（至少覆盖 `NSMicrophoneUsageDescription` 等已有的权限说明），并在 `Info.plist` 增加 `CFBundleLocalizations`（`en`、`zh-Hans`）和合适的 `CFBundleDevelopmentRegion`。注意这些系统弹窗跟随 macOS 系统语言，不随应用内设置。
- 更新 `AGENTS.md`：其中“Follow the existing Chinese interface copy.”改为“新增界面文案必须在两种语言同时添加并通过 `i18n::t` 读取”（`i18n/` 条目已在模块列表中）。
- `CHANGES.md`、必要时 `LESSONS.md`（例如遇到的 FFI、测试并行改全局语言、旧任务文件迁移等问题）、`docs/iteration-plan-2026-10-25.md` 第 10.5 节中标记各步完成状态。
- 如果仍保留 `Session::set_ui_language` 等之前的 `#[allow(dead_code)]`/`#[allow(unused_imports)]`（`i18n/mod.rs` 的 `t_args`），在其被使用后全部删除。

## 8. 验证与交付要求（每一步都要做）

自动化（均需通过，Clippy 必须零警告）：

```
cargo fmt --check
cargo clippy --all-targets --locked
cargo test --locked
```

每步都要：更新 `Cargo.toml` 版本（`cargo update --offline -p notype` 同步 `Cargo.lock`，否则 `--locked` 会失败）、在 `CHANGES.md` 按“新增功能/缺陷修复/工程治理与文档”记录、保持每一步可单独构建和 review。

手动验证（隔离数据目录，**绝不要指向生产数据**）：

1. `NOTYPE_DATA_DIR=/absolute/scratch/path cargo run --locked` 启动；该目录内的 `notype.db` 的 `settings` 表可写入 `ui_language`（值 `en` / `zh-Hans` / `system`）来模拟（I2 之后用界面选择器）。
2. 英文：全部界面为英文且无截断；中文：与改动前逐字一致；系统语言为中文时选“跟随系统”显示中文。
3. 运行中来回切换（I2 起），托盘菜单、窗口主体、浮动胶囊都应更新。
4. 回归 AGENTS.md 要求的 record → transcribe → polish → paste、取消、润色失败回退（需要真机权限与 Key，若无法执行必须在交付说明中明确“未验证”）。
5. 渲染不得有磁盘 I/O、系统语言查询或整表克隆；滚动设置页和历史列表确认无明显卡顿。

不要在测试里依赖主机语言：`detect.rs` 的测试只验证调用不 panic。解析规则测试已在 `lang.rs`。

## 9. 范围外与风险

- 不实现 `zh-Hant`、运行中跟随系统语言变化（下次启动生效）、复数引擎。
- 不改动润色提示词和 `mining.rs` 的算法数据。
- 账户阶段（P2）的界面和邮件模板届时直接使用词条，参见方案文档 3.1 与 10.7。
- 英文文案质量需母语者审校，发布前建议安排。
- 迁移量大（约 650 行含中文的 Rust 行，分布在 40 多个文件）；按文件逐个迁移并随时跑测试，避免一次大提交。
