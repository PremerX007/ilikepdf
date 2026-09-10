# Architecture

ilikepdf uses a layered dependency direction:

```text
Flutter presentation
        |
generated flutter_rust_bridge contract
        |
Rust bridge adapter -> Rust application core -> native/PDF infrastructure
```

## Boundaries

- `apps/desktop/lib/src/app/` owns widgets, interaction, and presentation state.
  It may call generated typed APIs but must not manipulate PDFs or invoke native
  libraries, subprocesses, PDFium, or qpdf.
- `crates/ilikepdf_bridge/` is the FFI boundary. It initializes native services,
  maps core models into stable Dart-facing types, and converts failures into
  structured application errors. Keep it thin.
- `crates/ilikepdf_core/` owns application rules and future jobs, filesystem
  operations, and PDF workflows. It has no Flutter dependency and is suitable
  for reuse from a future worker process.
- `crates/ilikepdf_pdf/` is the native PDF infrastructure adapter. Its public
  request/result types are engine-neutral; PDFium bindings and errors remain
  private and are mapped into stable core error categories.
- `third_party/pdfium/windows/x64/` contains the pinned, licensed runtime. CMake
  copies `pdfium.dll` beside the Windows executable and installs its notices.

The current PDF path is intentionally narrow:

```text
Flutter picker/widget -> typed bridge DTO -> core preview workflow
  -> temporary PNG -> ilikepdf_pdf -> PDFium 7881 -> atomic publish -> Flutter image
```

Opening and rendering are worker-pool calls from Dart. A later job boundary can
move the native adapter into a worker process without changing presentation
contracts. Do not expose PDFium document/page handles across crate or FFI
boundaries.

## PDF rendering runtime

PDFium is used because rendering fidelity is its intended role in the product;
structural operations will remain separate behind Rust boundaries in later
milestones. Dynamic binding keeps the native runtime replaceable and avoids
linking PDFium into the bridge. Windows x64 builds load the pinned DLL beside
the executable and never download or search for a system installation at
runtime. Development and native tests use the same vendored DLL from the source
tree.

The adapter pins `pdfium-render` to the released `pdfium_7881` ABI and bundles
PDFium `151.0.7881.0`. Exact artifact/DLL hashes, source URLs, build flags, and
licenses are recorded in `third_party/pdfium/README.md` and its adjacent notice
files. Upgrade the binding feature and runtime as one reviewed change.

## Privacy and file safety

Runtime PDF functionality must work without a network connection. Never add
telemetry, analytics, upload paths, or cloud fallbacks. Original documents are
read-only unless replacement is explicitly requested. Document-producing jobs
must write beside or within a controlled temporary location, validate success,
then atomically publish or replace the destination where the filesystem permits.

Logging accepts allow-listed event names only. Never log passwords, document
content, metadata, or paths. Errors crossing the bridge are typed variants with
safe user-facing messages.

Long-running Rust calls should remain asynchronous from Dart's perspective and
must add progress and cancellation when introduced. Keep native execution behind
boundaries that can later move into an isolated worker process.
