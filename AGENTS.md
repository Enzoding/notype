# Repository Guidelines

## Project Structure & Module Organization

Notype is a Rust 2021 macOS dictation application using GPUI Kit. All application code lives in `src/`:

- `main.rs`: startup, windows, tray menu, and event dispatch.
- `session/`: session state and recording lifecycle, divided into domain submodules (`lifecycle`, `settings`, `history`, `mining`, `studio`, `time`, `types`).
- `capture.rs`, `audio.rs`, `hotkey.rs`, `paste.rs`, `ax.rs`, and `front.rs`: audio, accessibility, clipboard, and frontmost application sensing.
- `providers/`: channel profiles, endpoint validation, model catalogs, HTTP retry/cancellation, and Vercel/OpenRouter transcription and chat protocols. One active channel owns transcription, polishing, and glossary calls; endpoint URLs are fixed by channel and are not editable. `gateway.rs` is a compatibility facade.
- `tasks/`: serializable recording jobs, raw-audio journal/checkpoints, recovery repository, and resumable transcription/polishing. Keep GPUI out of this feature.
- `i18n/`: interface language preference, macOS language detection, and embedded `locales/*.json` catalogs (`en`, `zh-Hans`). New user-visible copy must add the key to both catalogs and be read through `i18n::t`; never persist or branch on translated text.
- `polish.rs` and `mining.rs`: writing modes and terminology learning.
- `permissions.rs`: native macOS microphone authorization queries and prompts; cache permission results outside rendering.
- `store.rs`: SQLite history/settings persistence and idempotent task-to-history mapping. `secrets.rs` and `keychain.rs`: verified legacy credential migration and native macOS Keychain access.
- `views/`, `overlay.rs`, and `ui.rs`: workspace screens, modular UI controls (`views/components/`), floating voice capsule, and shared styling.

Unit tests are embedded beside implementation code. UI assets come from GPUI Kit. `target/` contains ignored build output.

## Build, Test, and Development Commands

Run on macOS with Rust and Apple developer tools:

- `cargo run --locked`: build and launch locally.
- `cargo build --release --locked`: produce an optimized executable in `target/release/`.
- `cargo test --locked`: run the unit test suite.
- `cargo test --locked mining::tests`: run terminology-learning tests only.
- `cargo fmt --check`: check standard Rust formatting; use `cargo fmt` to apply it (install via `rustup component add rustfmt` if missing).
- `NOTYPE_DATA_DIR=/absolute/scratch/path cargo run --locked`: isolated development data and a separate development Keychain service; does not import the project `.env.local`. Never point this at production data for smoke tests.
- `cargo clippy --all-targets --locked`: inspect compiler and lint diagnostics (install via `rustup component add clippy` if missing).

## Build Disk Budget

- Development and test builds use line-table debug information and disable incremental compilation in `Cargo.toml`. Keep release packaging settings independent. Full variable debugging can be enabled temporarily with `CARGO_PROFILE_DEV_DEBUG=2`; clean afterward because profile changes can leave old artifacts.
- Run `bash scripts/build-cache.sh report` before and after build configuration/dependency changes. Run `bash scripts/build-cache.sh check` at handoff after builds; the default local `target/` budget is 4 GiB (override with `NOTYPE_BUILD_BUDGET_MIB`). This is a maintenance alert, not a hard filesystem quota.
- Prefer `cargo check --locked` for compile-only feedback; build/test when needed. Avoid unnecessary custom target directories, profiles and repeated release builds, which accumulate separate artifacts.
- If over budget, stop active builds and use `bash scripts/build-cache.sh clean-dev` or `clean-all`, then rebuild only the required profile. Never clean automatically during a build or app startup. Cleanup costs a subsequent full compilation.
- The script is restricted to this checkout's `target/`; it does not clean shared Cargo/toolchain caches, `release/`, recordings, databases or Keychain. Custom `CARGO_TARGET_DIR` locations must be measured and maintained separately.
- See `docs/build-disk.md` for tradeoffs, baseline and commands. Record new measurements after significant build/dependency changes rather than treating the budget as guaranteed.

## UI Rendering & GPUI Performance Guidelines

Notype runs at native 60~120Hz display refresh rates on macOS. Uphold these strict UI invariants:

- **Render Purity & Zero Disk I/O**: `Render::render` must be pure memory layout computation. Never perform filesystem calls (`is_file`, `metadata`, `read_dir`) or synchronous I/O inside render paths; maintain indices and presence flags in memory (`Session::audio_ids`, etc.).
- **GPUI Scroll Loop Awareness**: GPUI's `overflow_y_scroll` triggers `cx.notify` on each scroll offset tick. Avoid deep-cloning large vectors (borrow references where practical) and avoid $O(M \times N)$ linear searches inside render loops (pre-index with `HashSet`).
- **Dynamic List Pagination**: For unbounded collections like dictation history and vocabulary, limit initial rendering (e.g. 25-50 items) and provide load-more controls to prevent mounting thousands of unvirtualized Flexbox nodes.
- **GPUI Group Isolation**: When using `group_hover` on list items, every row must use an isolated unique group identifier (`SharedString::from(format!("phrase-row-{id}"))`). Never share a static group name across repeated rows to prevent global window mouse listener storms and Hitbox collisions.

## Coding Style & Naming Conventions

Use standard rustfmt formatting with four-space indentation. Use `snake_case` for modules, functions, and variables; `PascalCase` for types; and `SCREAMING_SNAKE_CASE` for constants. Keep macOS FFI isolated in platform modules and document unsafe assumptions. Add new user-visible copy to both `locales/en.json` and `locales/zh-Hans.json`, keeping English in idiomatic macOS editorial style and preserving Chinese interface conventions. Run blocking network and expensive work through background execution so the UI remains responsive.

Before adding complexity for a speculative or vanishingly unlikely race or security edge case, explain the concrete failure mode, likelihood, and complexity cost to the user and get their buy-in. Do not silently expand scope for theoretical robustness.

## Testing Guidelines

Use Rust's built-in `#[test]` framework inside `#[cfg(test)] mod tests`. Name tests after observable behavior, such as `learns_term_and_correction_from_edit`. Use in-memory SQLite for storage tests. No coverage threshold is configured. Manually verify microphone permissions, global shortcuts, clipboard insertion, and UI changes; unit tests do not exercise these system flows.

## Development & Architecture Maintenance

Apply responsibility and maintainability checks to every source file, module, and type, including UI components, storage, AI integration, terminology learning, and platform code. Before extending existing code, inspect its responsibilities and dependencies. If the change introduces an independent concern or exposes existing coupling, extract a cohesive module as part of the work. Do not continually append functionality to convenient central files or shared helper modules.

Split by responsibility and reason for change, rather than arbitrary line counts. Warning signs include unrelated features modifying the same module, many independent state fields, duplicated rules, broad imports, and tests requiring unrelated infrastructure. A split must reduce coupling: moving methods into separate files while passing the entire application state does not establish a useful boundary.

Use feature directories with private submodules when a feature grows. Keep `mod.rs` focused on a small public interface and composition. Name modules after their domain responsibility; avoid catch-all `utils`, `helpers`, or `common` modules. Keep state with its owning feature, expose explicit inputs and outputs, and use the narrowest practical visibility. Avoid cyclic dependencies and abstractions without a concrete need.

Maintain these boundaries as the implementation evolves:

- Application coordination: recording lifecycle, task sequencing, cancellation, and result delivery; delegate feature rules and infrastructure operations.
- UI: screen composition, presentation state, and interaction handling; extract reusable controls by behavior, keeping business rules outside rendering.
- AI integration: separate HTTP transport, request/response types, and transcription, polishing, or glossary operations when they evolve independently.
- Persistence: separate schema/migrations and repositories for history, settings, phrases, and corrections as complexity grows; keep SQL outside views and workflow logic.
- Terminology learning: separate extraction, validation, ranking, edit comparison, and correction application when independently testable.
- Platform integration: isolate audio devices, hotkeys, accessibility, clipboard, and window management behind focused interfaces.

Keep pure domain logic independent of GPUI, HTTP, SQLite, and macOS APIs where practical. During extraction, move relevant tests with the behavior, preserve public behavior, and remove obsolete paths and duplication. Verify callers and document changed boundaries; each refactoring step should build and remain reviewable. Apply this incrementally across affected modules rather than requiring an unrelated repository-wide rewrite for every feature.

Before implementing a feature, trace its state transitions and failure paths. Preserve cancellation and generation checks so stale background results cannot update a newer recording. Keep refactoring scoped and behavior-preserving; describe any intentional behavior changes separately.

For behavioral fixes, add focused regression tests where practical. Cover empty input, malformed AI responses, failed requests, and persistence behavior as relevant. Run formatting, applicable tests, and Clippy before handing off; report failures and unverified system flows explicitly. Changes to dictation require a manual record → transcribe → polish → paste check, including cancellation and polish-failure fallback.

## Version Management Guidelines

- **SemVer Compliance**: Follow Semantic Versioning (`MAJOR.MINOR.PATCH`).
- **Update Version on Every Change**: Every functional addition, UI adjustment, refactoring, or bug fix must bump the version in `Cargo.toml`.
  - For small incremental changes, behavioral fixes, and UI tweaks: bump the `PATCH` version (e.g. `0.1.0` -> `0.1.1`).
  - For backwards-compatible feature additions: bump the `MINOR` version (e.g. `0.1.0` -> `0.2.0`).
- **Synchronized Packaging**: The macOS packaging script (`scripts/package.sh`) dynamically reads `Cargo.toml`'s version. Bumping `Cargo.toml` automatically reflects in the macOS bundle `Info.plist` (`CFBundleShortVersionString`, `CFBundleVersion`) and the output DMG name (`Notype-<version>.dmg`).

## Documentation & Knowledge Base Guidelines

Every AI agent and contributor working on this repository MUST uphold these two living documents:

1. **`CHANGES.md` (Changelog)**:
   - Must be updated after **every** code change or release.
   - Record the version number, release date, and categorize items into:
     - `新增功能 (Features)`
     - `缺陷修复 (Fixes)`
     - `工程治理与文档 (Engineering & Documentation)`
   - Used for release notes, user audit, and git history tracking.

2. **`LESSONS.md` (Failure Post-Mortem & Troubleshooting Knowledge Base)**:
   - When encountering a difficult bug, network issue, platform quirk (macOS permissions, proxy, FFI, audio), or recurring developer mistake:
   - **Immediately document it in `LESSONS.md`** under the structure:
     - `问题现象` (Symptoms)
     - `根本原因` (Root Cause)
     - `防范与解决方案` (Prevention & Solution)
   - Consult `LESSONS.md` before making architectural or platform changes to avoid repeating past mistakes.

## Operations & Ongoing Maintenance

This is a local desktop application with an external AI dependency. Diagnose failures by stage: microphone capture, permissions/hotkey handling, Gateway transcription, polishing, insertion, or persistence. Keep diagnostics useful without logging API keys, audio, or dictated text. Verify model and API changes through connection testing and a short dictation before release.

Preserve existing user data when changing SQLite schemas or settings. Use backward-compatible migrations and test opening an older schema. Back up the database and recordings before manual migration or cleanup; do not delete user history as a troubleshooting shortcut. Document configuration-path or data-format changes and recovery steps in the PR.

Update this guide when commands, module boundaries, configuration, or release procedures change. Keep dependency upgrades focused and review `Cargo.lock` changes. No automated release or operational monitoring workflow is currently defined; document any new process alongside its implementation.

## Commit & Pull Request Guidelines

Recent commits use prefixes such as `feat(dictionary):`, `feat(ui):`, and `refactor:`. Use concise imperative descriptions. PRs should explain the problem, resulting behavior, and validation performed; link relevant issues and include screenshots or recordings for visual changes. Mention persistence or permission changes.

## Security & Configuration

New user-entered channel keys are stored in macOS Keychain. Legacy SQLite / `.env.local` credentials are migrated only after a verified Keychain write; unrelated environment assignments are preserved. For first-time developer setup, `.env.example` / `AI_GATEWAY_API_KEY` remains a legacy import source. Never commit keys or expose them in logs. Production and isolated development Keychain services are separate. Local history and optional recordings live under `~/Library/Application Support/Notype/` (with automatic backwards compatibility fallback for legacy `~/Library/Application Support/OpenTypeless/`). Audio and text are sent to AI Gateway for processing; avoid real user data in fixtures.

## Recording Recovery

Recordings stop automatically after ten minutes. Audio is journaled to `tasks/<id>/audio.raw` (interleaved little-endian f32 with format in `task.json`); the atomically written `checkpoint` contains the last synced byte count. Recovery consumes only complete frames up to that count, so a crash can lose the most recent unsynced tail. ASR uses contiguous 30-second chunks and checkpoints each successful transcript. Failed/interrupted jobs are retained for seven days and cleaned on a recovery scan; successful jobs are removed after history/audio archival. Recovery is explicit and never auto-pastes. Retrying uses current route/model configuration for unfinished stages while preserving completed chunks. Back up the entire data directory before manual repairs; do not edit or delete a running job.

HTTP timeouts are ambiguous and are not automatically resent. Only 429/502/503/504 receive bounded automatic retries (two retries; short numeric Retry-After honored). Cancellation stops subsequent calls and delivery; an already running blocking request may still finish and incur upstream charges.

## Setup & Preferences Presentation

The workbench's first module shows exactly two setup requirements: whether the selected channel has a key and whether microphone plus Accessibility permissions are complete. Show OK when both are satisfied. Keep model IDs, endpoint addresses, connection test results, and device availability out of this first module. Preferences mount only the selected topic; model catalogs are opt-in and bounded to six entries per type. Never query permissions, detect proxies, scan files, or clone full catalogs / recording texts during a scroll render.
