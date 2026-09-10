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
  operations, and PDF workflows. It has no Flutter dependency and is suitable for
  reuse from a future worker process.
- Future PDF engines belong behind operation-focused Rust interfaces in native
  infrastructure modules. Engine details must not cross into the bridge contract.

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

