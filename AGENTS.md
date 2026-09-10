# Repository Guidelines

## Project Structure & Boundaries

`apps/desktop/` contains the Windows Flutter UI. Keep widgets and presentation
state under `lib/src/app/`; generated Dart bridge code lives under
`lib/src/rust/`. `crates/ilikepdf_bridge/` is the thin Dart/Rust adapter, while
`crates/ilikepdf_core/` owns application rules and future filesystem, job, and
PDF workflows. Tests sit beside their ecosystem (`test/`, `integration_test/`,
or Rust `#[cfg(test)]` modules). See `docs/architecture.md` before adding a new
layer or dependency.

Flutter code must never manipulate PDFs, start subprocesses, or invoke PDFium,
qpdf, or other native libraries directly. Put native implementations behind
operation-focused Rust modules; do not create catch-all `utils`, `helpers`,
`pdf.rs`, or `service.rs` files.

## Build, Test, and Development Commands

From `apps/desktop/`, run `flutter pub get`, then `flutter run -d windows` for
development. After changing public Rust API files, run
`flutter_rust_bridge_codegen generate` and commit all generated changes.

Before opening a pull request, run:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `dart format --output=none --set-exit-if-changed lib test integration_test hook`
- `flutter analyze`, `flutter test`, `flutter build windows`, and (with a Windows
  toolchain) `flutter test integration_test/app_info_test.dart -d windows`

## Coding and Testing Conventions

Use standard `rustfmt` and Dart formatting. Rust modules and Dart files use
`snake_case`; Rust/Dart types use `UpperCamelCase`; Dart members and Rust
functions use their language-standard `lowerCamelCase` and `snake_case`.
Generated bridge files are not hand-edited. Add deterministic tests for every
behavior change and regression; tests must not depend on network access or real
private documents.

## Privacy, Files, and Errors

Documents never leave the machine. Core features must work offline. Never log
passwords, document content, metadata, or paths. Treat originals as read-only
unless replacement is explicit; write outputs to a controlled temporary file,
validate success, then publish atomically where possible. Rust/Dart failures use
structured error variants, not string matching. Design long operations for
progress, cancellation, and future worker-process isolation.

## Commits and Pull Requests

Use concise imperative subjects, preferably `feat:`, `fix:`, `docs:`, or
`test:`. Keep pull requests focused; explain the problem and solution, list
validation, link issues, and include screenshots for UI changes. Call out new
dependencies, native code, security impact, and breaking bridge changes.

When Codex implemented or materially contributed to a commit, create it with
`git cc` so it appends this trailer while preserving the human contributor as
the primary author and committer:

`Co-authored-by: Codex <codex@openai.com>`

Do not add Codex attribution to commits without a material Codex contribution.
Never change repository or global `user.name` or `user.email` for attribution.
