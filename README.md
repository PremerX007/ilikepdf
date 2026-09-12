# ilikepdf

ilikepdf is an open-source, privacy-first Windows PDF toolkit. Flutter provides
the desktop presentation layer while Rust owns application logic and native PDF
integration. Documents and images are processed locally and are never uploaded.

Current production workflows include batch PDF-to-PNG/JPG export and Image-to-PDF
creation. PDF-to-Images accepts ordered multi-file picker/drop input, exports
sequentially as PNG or JPG at 150 or 300 DPI, supports per-source or shared custom destinations,
continues after document-specific failures, and numbers output files or folders
instead of overwriting existing data. Image-to-PDF accepts JPG/JPEG, PNG, and
WebP images and supports ordered merged or separate output. The desktop home and
two-column tool workspace form the reusable visual shell for future local PDF
tools.

## Prerequisites

- [Flutter 3.47.3](https://docs.flutter.dev/platform-integration/windows/building)
  with Windows desktop support
- Rust 1.98.1 for `x86_64-pc-windows-msvc` (also pinned in the repository)
- Visual Studio with the **Desktop development with C++** workload, CMake tools,
  and a Windows SDK
- [`flutter_rust_bridge_codegen` 2.13.0](https://cjycode.com/flutter_rust_bridge/quickstart)
  when changing the Rust bridge API

## Development

```powershell
cd apps/desktop
flutter pub get
flutter_rust_bridge_codegen generate
flutter run -d windows
```

Generated bridge files under `apps/desktop/lib/src/rust/` and
`crates/ilikepdf_bridge/src/frb_generated.rs` are committed. Regenerate them in
the same change as any public API update.

Run the quality gates from the repository root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cd apps/desktop
dart format --output=none --set-exit-if-changed lib test integration_test hook
flutter analyze
flutter test
flutter build windows --release
flutter test integration_test/app_info_test.dart -d windows
flutter test integration_test/pdf_preview_test.dart -d windows
flutter test integration_test/image_to_pdf_test.dart -d windows
```

See [docs/architecture.md](docs/architecture.md) for boundaries and privacy
invariants. A project license must be selected before the first public release.
