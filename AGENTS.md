# Repository Guidelines

## Project Structure & Boundaries

`apps/desktop/` contains the Windows Flutter UI; keep widgets and presentation
state under `lib/src/app/` and generated bridge code under `lib/src/rust/`.
`crates/ilikepdf_bridge/` is the thin FFI adapter, `crates/ilikepdf_core/` owns
application and filesystem workflows, and `crates/ilikepdf_pdf/` isolates native
PDF engines. Vendored runtimes and notices live under `third_party/`. Keep tests
in each ecosystem's `test/`, `integration_test/`, or Rust test modules. Read
`docs/architecture.md` before adding a layer or dependency.

Flutter must never manipulate PDFs, start subprocesses, or invoke PDFium, qpdf,
or native libraries directly. Use operation-focused Rust modules; do not create
catch-all `utils`, `helpers`, `pdf.rs`, or `service.rs` files.

## Build, Test, and Development Commands

From `apps/desktop/`, run `flutter pub get` and `flutter run -d windows`. After
changing public Rust APIs, run `flutter_rust_bridge_codegen generate` and commit
all generated changes. Before a pull request, run:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `dart format --output=none --set-exit-if-changed lib test integration_test hook`
- `flutter analyze`, `flutter test`, and `flutter build windows --release`
- Windows integration tests in `integration_test/`, including `app_info_test.dart`
  and `pdf_preview_test.dart`

Codex and the interactive Windows user can have isolated views of the user-wide
Pub cache even when both paths display as `%LOCALAPPDATA%\Pub\Cache`. After Codex
adds or updates a hosted Flutter dependency and finishes its own validation, run
`flutter clean` as the final handoff step and do not regenerate `.dart_tool` or
`build/` afterward. This ensures the user's next direct `flutter build` or
`flutter run` resolves packages in the user's own environment instead of reusing
Codex-generated package metadata that points into Codex's cache view.

## Coding, Testing, and Native Dependencies

Use standard `rustfmt` and Dart formatting. Files/modules use `snake_case`;
types use `UpperCamelCase`; members follow each language's conventions. Never
hand-edit generated bridge files. Add deterministic offline tests for behavior
changes; never use real private documents. Pin native runtimes by version,
architecture, source URL, SHA-256, and license notices. Upgrade a runtime and
its binding feature together, with automatic release packaging.

## Privacy, Files, and Errors

Documents never leave the machine and core features must work offline. Never
log passwords, content, metadata, or paths. Treat originals as read-only unless
replacement is explicit. Write output to a controlled temporary file, validate
success, then publish atomically where possible. Use structured error variants,
not string matching. Design long operations for progress, cancellation, and
future worker-process isolation.

## Commits and Pull Requests

Use concise imperative subjects, preferably `feat:`, `fix:`, `docs:`, or
`test:`. Focus pull requests; describe the problem and solution, list validation,
link issues, include UI screenshots, and call out dependency, native, security,
or bridge-contract changes.

For commits Codex materially contributed to, use `git cc` to append
`Co-authored-by: Codex <codex@openai.com>` while preserving the human as primary
author and committer. Omit attribution otherwise. Never change repository or
global `user.name` or `user.email` for attribution.
