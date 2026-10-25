# Repository Guidelines

## Project Structure & Module Organization

Notype is a Rust 2021 macOS dictation application using GPUI Kit. All application code lives in `src/`:

- `main.rs`: startup, windows, tray menu, and event dispatch.
- `session/`: session state and recording lifecycle, divided into domain submodules (`lifecycle`, `settings`, `history`, `mining`, `studio`, `time`, `types`).
- `capture.rs`, `audio.rs`, `hotkey.rs`, `paste.rs`, `ax.rs`, and `front.rs`: audio, accessibility, clipboard, and frontmost application sensing.
- `gateway.rs`, `polish.rs`, and `mining.rs`: AI requests, writing modes, and terminology learning.
- `store.rs` and `secrets.rs`: SQLite persistence and API key configuration.
- `views/`, `overlay.rs`, and `ui.rs`: workspace screens, modular UI controls (`views/components/`), floating voice capsule, and shared styling.

Unit tests are embedded beside implementation code. UI assets come from GPUI Kit. `target/` contains ignored build output.

## Build, Test, and Development Commands

Run on macOS with Rust and Apple developer tools:

- `cargo run --locked`: build and launch locally.
- `cargo build --release --locked`: produce an optimized executable in `target/release/`.
- `cargo test --locked`: run the unit test suite.
- `cargo test --locked mining::tests`: run terminology-learning tests only.
- `cargo fmt --check`: check standard Rust formatting; use `cargo fmt` to apply it (install via `rustup component add rustfmt` if missing).
- `cargo clippy --all-targets --locked`: inspect compiler and lint diagnostics (install via `rustup component add clippy` if missing).

## UI Rendering & GPUI Performance Guidelines

Notype runs at native 60~120Hz display refresh rates on macOS. Uphold these strict UI invariants:

- **Render Purity & Zero Disk I/O**: `Render::render` must be pure memory layout computation. Never perform filesystem calls (`is_file`, `metadata`, `read_dir`) or synchronous I/O inside render paths; maintain indices and presence flags in memory (`Session::audio_ids`, etc.).
- **GPUI Scroll Loop Awareness**: GPUI's `overflow_y_scroll` triggers `cx.notify` on each scroll offset tick. Avoid deep-cloning large vectors (borrow references where practical) and avoid $O(M \times N)$ linear searches inside render loops (pre-index with `HashSet`).
- **Dynamic List Pagination**: For unbounded collections like dictation history and vocabulary, limit initial rendering (e.g. 25-50 items) and provide load-more controls to prevent mounting thousands of unvirtualized Flexbox nodes.
- **GPUI Group Isolation**: When using `group_hover` on list items, every row must use an isolated unique group identifier (`SharedString::from(format!("phrase-row-{id}"))`). Never share a static group name across repeated rows to prevent global window mouse listener storms and Hitbox collisions.

## Coding Style & Naming Conventions

Use standard rustfmt formatting with four-space indentation. Use `snake_case` for modules, functions, and variables; `PascalCase` for types; and `SCREAMING_SNAKE_CASE` for constants. Keep macOS FFI isolated in platform modules and document unsafe assumptions. Follow the existing Chinese interface copy. Run blocking network and expensive work through background execution so the UI remains responsive.

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

## Operations & Ongoing Maintenance

This is a local desktop application with an external AI dependency. Diagnose failures by stage: microphone capture, permissions/hotkey handling, Gateway transcription, polishing, insertion, or persistence. Keep diagnostics useful without logging API keys, audio, or dictated text. Verify model and API changes through connection testing and a short dictation before release.

Preserve existing user data when changing SQLite schemas or settings. Use backward-compatible migrations and test opening an older schema. Back up the database and recordings before manual migration or cleanup; do not delete user history as a troubleshooting shortcut. Document configuration-path or data-format changes and recovery steps in the PR.

Update this guide when commands, module boundaries, configuration, or release procedures change. Keep dependency upgrades focused and review `Cargo.lock` changes. No automated release or operational monitoring workflow is currently defined; document any new process alongside its implementation.

## Commit & Pull Request Guidelines

Recent commits use prefixes such as `feat(dictionary):`, `feat(ui):`, and `refactor:`. Use concise imperative descriptions. PRs should explain the problem, resulting behavior, and validation performed; link relevant issues and include screenshots or recordings for visual changes. Mention persistence or permission changes.

## Security & Configuration

Copy `.env.example` to `.env.local` and set `AI_GATEWAY_API_KEY`; never commit keys or expose them in logs. Configuration resolves relative to the compile-time project directory. Local history and optional recordings live under `~/Library/Application Support/OpenTypeless/`. Audio and text are sent to AI Gateway for processing; avoid real user data in fixtures.
