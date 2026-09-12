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

Rust unit tests remain `#[cfg(test)]` child modules under `src/` so they can
exercise private implementation details without widening production visibility.
To keep production modules readable, a module such as `foo.rs` declares only
`mod tests;`, while its unit-test implementation lives in `foo/tests.rs`.
Crate-level `tests/` directories remain reserved for integration tests that use
the same public API available to external consumers.

`ilikepdf_core::lib.rs` is the only supported public façade for application
workflows. Internal module paths are intentionally private so feature code can be
reorganized without creating a second API or changing bridge consumers.

### Rust change map

| Change | Primary owner |
| --- | --- |
| Dart-visible request, result, progress, or error shape | `ilikepdf_bridge/src/api/` |
| PDF preview, single export, or batch behavior | `ilikepdf_core/src/application/pdf_to_images/` |
| Image-to-PDF workflow and output naming | `ilikepdf_core/src/application/image_to_pdf/` |
| Destination validation, collision policy, and atomic publication | `ilikepdf_core/src/application/output/` |
| PDFium page rendering and document inspection | `ilikepdf_pdf/src/pdfium_engine.rs` |
| PNG/JPG encoding policy | `ilikepdf_pdf/src/pdfium_engine/image_encoding.rs` |
| Image decoding, layout, and PDFium image placement | `ilikepdf_pdf/src/image_pdf/` |
| Bundled PDFium loading | `ilikepdf_pdf/src/runtime.rs` |

The PDF paths are intentionally separated by purpose:

```text
Flutter picker/widget -> typed bridge DTO -> core preview workflow
  -> temporary PNG -> ilikepdf_pdf -> PDFium 7881 -> atomic publish -> Flutter image

Flutter PDF batch workspace -> typed batch progress stream -> core orchestrator
  -> documents in card order -> one page at a time at 150/300 DPI
  -> PNG or JPG encoding -> atomic non-clobber publish
  -> per-document result -> mixed-success batch summary

Flutter image arranger -> typed progress stream -> core Image-to-PDF job
  -> one decoded/oriented image at a time -> engine-neutral page layout
  -> PDFium 7881 document/page/image objects -> private temporary PDF
  -> atomic non-clobber publish -> progress/completion/structured failure
```

Preview stays width-based for display use. Production export derives each page's
pixel dimensions from its rotated PDF point dimensions and selected DPI. Export is
sequential so rendered page memory and native resources are released before the
next page begins; completed output paths remain available if a later page fails.

PDF-to-image accepts one or more PDFs through a shared picker/drop ingestion path.
The card order is the sequential processing order; reordering cards never changes
page order inside a document. Each card uses the preview pipeline only for a
compact page-1 thumbnail, while production export continues to render each page
independently at Standard 150 DPI or High 300 DPI. Export format is an independent
typed choice: PNG is the default lossless output, while JPG uses fixed quality 90.
Preview rendering remains PNG-only and never determines the export format.
Production PNG uses the `png` crate's fast lossless encoder. JPG uses a
runtime-dispatched SIMD encoder with 4:4:4 chroma sampling so colored text and
graphics retain full chroma resolution. Both encoders consume PDFium's RGBA
buffer directly where supported and write through a buffered stream before the
temporary output is flushed and atomically published.

The default `Next to source files` destination resolves a separate base directory
from each source PDF. `Custom folder` uses one selected base directory for every
document and survives add, remove, and reorder interactions. Output structure is
based only on each document's page count: a one-page document writes
`<source-stem>-page-0001.<format>` directly into its base directory; a multi-page
document creates `<base>/<source-stem>/` and writes the original-stem page names
inside it. The selected extension is `.png` or `.jpg`; page numbers use at least
four digits and valid Unicode stems are preserved.

Existing outputs are never replaced. A one-page filename collision uses the
lowest available suffix such as `cover-page-0001 (1).png` or
`cover-page-0001 (1).jpg`; a multi-page folder
collision similarly uses `invoice (1)` while its internal filenames remain
`invoice-page-0001.<format>`, `invoice-page-0002.<format>`, and so on. Allocation compares
names case-insensitively for Windows and publishes with atomic no-clobber
operations, retrying when another writer wins a race.

Batch planning inspects documents to obtain the overall page total, then exports
sequentially so memory remains bounded to a page render and one document's native
state. A document-specific open, render, encode, or publication failure records
that document's structured result and processing continues with the next card.
Global preconditions, including an unavailable PDFium runtime or invalid shared
custom destination, stop the batch. Final results retain ordered per-document
successes, failures, published outputs, and partial page counts.

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

## Image-to-PDF behavior

Image-to-PDF accepts `.jpg`, `.jpeg`, `.png`, and `.webp`. Both the extension and
actual decodability are validated. Phase 1A.3 does not support TIFF, animated or
multi-frame image semantics, image editing, OCR, compression controls, or page
sizes beyond Fit, A4, and US Letter. Source metadata is read only when necessary
for visual orientation and is not copied into the generated PDF.

Each selected image creates exactly one page in the displayed order. Fit mode
uses the visually oriented pixel dimensions and a deterministic logical 96 PPI
mapping (`pixels / 96 * 72` points); it always has zero margin and ignores the
stored standard-page orientation choice. A4 uses 210 x 297 mm and US Letter uses
215.9 x 279.4 mm, swapped for landscape. Standard pages support 0 mm, 10 mm, or
20 mm equal margins. Images are centered in the available content box, scaled up
or down without distortion, and are never cropped.

Merged mode creates one PDF whose name uses the stem of the first image in the
final conversion order, regardless of the number of images. Reordering therefore
changes both page order and the merged output name; `images.pdf` is not used.
Separate mode starts from `<image-stem>.pdf` for each image directly in the
selected directory. Duplicate stems and existing output names are resolved in
current image order with the lowest available ` (n)` suffix, such as `scan.pdf`,
`scan (1).pdf`, and `scan (2).pdf`. Name matching is case-insensitive to match
Windows filesystem behavior. The first selected image's parent directory is the
initial destination; an explicitly selected custom destination survives adding,
removing, and reordering images.

Existing outputs are never overwritten. Image-to-PDF publishes through an atomic
no-clobber operation and retries with the next available running number if a name
becomes occupied between allocation and publication. Merged output is published
only after the complete document is saved successfully. Separate output validates
every input before publication, then creates PDFs sequentially; structured
failures include already-published paths if a later filesystem operation fails.
Full-resolution decoded buffers are held one image at a time and released after
placement.

## Desktop tool workspace

The desktop presentation opens on a home grid whose enabled cards navigate to
implemented tools. Placeholder cards are visibly marked as coming soon and have
no navigation action. Tool pages use a shared presentation scaffold: a scrollable
file/page workspace on the left and a stable, tool-specific settings inspector on
the right, with the primary action anchored at the inspector bottom. At narrow
desktop widths the inspector moves below the workspace. Shared widgets accept
typed content and callbacks and do not own conversion, validation, or filesystem
state.

Ordered workflows use reusable preview cards and a reorderable grid; the owning
tool remains the single source of ordering state. Image-to-PDF wraps both empty
and populated workspaces in one native file-drop target. Picker paths and dropped
paths pass through the same `ImageToPdfWorkflow.prepareImagePaths()` ingestion
method before entering the same core conversion path. Native Windows Explorer
drop events are supplied by the pinned `desktop_drop 0.8.4` package because
`file_selector` provides native pickers but not a desktop drop target.

PDF-to-Images uses the same drop target, card, reorder grid, destination picker,
and anchored settings/action structure. Picker and Windows Explorer drop paths
both pass through `PdfToImageWorkflow.preparePdfPaths()`. The populated workspace
supports adding and removing PDFs without resetting a custom destination, and
the result region summarizes completed and failed documents without colliding
with the primary action.
