# Architecture

ilikepdf uses a layered dependency direction:

```text
Flutter presentation
        |
generated flutter_rust_bridge contract
        |
Rust bridge adapter -> Rust application core -> native PDF infrastructure
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
- `crates/ilikepdf_qpdf/` is the current structural-PDF infrastructure adapter.
  It translates the core capability contract into direct qpdf CLI invocations,
  owns process execution and bundled-runtime resolution, and exposes no process
  details through the application boundary.
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
| Read-only editor sessions, page geometry, and coordinate conversion | `ilikepdf_core/src/application/editor/` |
| Native page-box and rotation inspection | `ilikepdf_pdf/src/pdfium_engine/page_geometry.rs` |
| PNG/JPG encoding policy | `ilikepdf_pdf/src/pdfium_engine/image_encoding.rs` |
| Image decoding, layout, and PDFium image placement | `ilikepdf_pdf/src/image_pdf/` |
| Bundled PDFium loading | `ilikepdf_pdf/src/runtime.rs` |
| Structural validation and safe rewrite orchestration | `ilikepdf_core/src/application/structural_pdf/` |
| qpdf CLI semantics, process execution, and runtime lookup | `ilikepdf_qpdf/src/` |

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

## PDF editor foundation (Phase 2A.1)

The editor foundation introduces no production Edit PDF route, widgets, editing
operations, viewport controller, undo/redo, or save/export workflow. Existing
preview, conversion, and structural tools retain their contracts and behavior.
The boundary for future presentation is:

```text
Flutter/editor presentation (future; logical local coordinates)
        |
typed bridge adapter (future; no editor API exposed in this milestone)
        |
EditorSession / PageGeometry / PageTransform (application/core)
        |
engine-neutral PdfDocumentGeometry inspection (PDF infrastructure facade)
        |
private PDFium page/box/rotation APIs
```

`open_editor_session()` obtains every page's geometry in source order with one
read-only document open. `EditorSession` is an immutable metadata snapshot with
a process-local typed `EditorSessionId`, source path, `ReadOnly` state, page count
derived from its ordered page list, and zero-based `EditorPageMetadata`. Identity
is stable when that snapshot is cloned and different on a new open. Sessions
retain no native document/page handles, bitmaps, passwords, or output files; each
native page is released after inspection. They have no source-path Debug
representation. Reopening a source in a future workflow must revalidate it;
retaining a path does not guarantee the file has remained unchanged.

`PageGeometry` contains validated `PageBox` values for the effective visible box
and optional normalized, non-degenerate declared MediaBox/CropBox, plus intrinsic
clockwise rotation (0, 90, 180, 270). Width/height in points derive from the
unrotated visible box; display dimensions swap for 90/270. Full media dimensions
are available from the optional declared MediaBox. Private fields and constructors
prevent invalid boxes or stale derived dimensions. No PDFium type crosses the
infrastructure boundary.

Use the resolved visible box as the authority for rendering/edit alignment.
`FPDF_GetPageBoundingBox` returns the effective MediaBox/CropBox intersection,
including inherited attributes. Individual dictionary box getters can omit
inherited entries, so a missing declared box does **not** imply a zero origin or
absence of an effective crop. Declared boxes are provenance, not transform input.
The pinned native engine normalizes reversed box bounds, defaults a missing/empty
MediaBox to Letter, and defaults an absent/empty CropBox to the effective MediaBox.
Inspection rejects a non-positive effective box (including disjoint boxes).
Core also checks the native rotated dimensions against its derived dimensions,
allowing the native API's f32 rounding tolerance. These behaviors are tested
against the bundled PDFium, not inferred from declared boxes. See the primary
[PDFium box API](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdfview.h)
and [page dimension implementation](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/core/fpdfapi/page/cpdf_page.cpp);
the wrapper's description of its "bounding" box as painted-content bounds must
not be used for this workflow.

### Coordinate contract and transform

Canonical editing coordinates are **unrotated source PDF points**, with positive
x rightward and positive y upward. The PDF user-space origin remains `(0, 0)`;
the effective visible page may start at a positive or negative offset. Do not
renormalize persisted coordinates to a crop corner. Viewport coordinates are
presentation-only local Flutter logical units, with positive x rightward and
positive y downward. They have no fixed DPI or Windows pixel meaning.

Every future text, image, signature, annotation, or form feature must share core's
`PageTransform`; no feature may supply its own PDFium-specific or tool-specific
coordinate system. Given visible box `(L, B, R, T)`, let `W = R-L`, `H = T-B`,
`u = x-L`, `v = y-B`. Rotation yields top-left display coordinates:

| Intrinsic rotation | `(a, b)` from PDF | `(u, v)` from display |
| --- | --- | --- |
| 0 | `(u, H-v)` | `(a, H-b)` |
| 90 clockwise | `(v, u)` | `(b, a)` |
| 180 | `(W-u, v)` | `(W-a, b)` |
| 270 clockwise | `(H-v, W-u)` | `(W-b, H-a)` |

For actual displayed page rectangle `(left, top, width, height)`, `sx` and `sy`
are its dimensions divided by the rotated dimensions in PDF points. Forward
conversion is `(left + sx*a, top + sy*b)`; inverse conversion first subtracts
the rectangle origin and divides by each scale, applies the inverse row above,
then adds `(L, B)`. `PageTransform::at_scale` builds an aspect-preserving rectangle
from logical units per PDF point. `PageTransform::new` uses the exact displayed
rectangle, accommodating independently rounded raster dimensions. Presentation
must pass the rectangle occupied by the page image, excluding padding, and the
point in that same local coordinate space. No widget viewport work is included.

Boxes and display rectangles require finite positive dimensions. Conversion
rejects non-finite points and points outside the closed visible/displayed page;
only numerical boundary drift of at most `1e-7` PDF points is clamped. Errors are
typed `PageGeometryError` values without document diagnostics or paths. Future
off-page placement policy can be designed explicitly when needed.

### Verification and limits

Deterministic transform tests independently specify every corner mapping and
round-trip corners, center, and asymmetric points for all four rotations, A4,
Letter, wide/tall pages, positive/negative origins, six display scales, shifted
display rectangles, and independently rounded axis scales. Invalid geometry,
non-finite values, out-of-page inputs, and boundary tolerance are covered.

`tests/fixtures/generate_geometry_fixtures.cjs` uses only Node's standard library
to reproduce project-owned fixtures. PDF infrastructure integration tests inspect
real native geometry. Core's native session tests verify ordered page metadata,
identities, empty/invalid PDFs, native dimension disagreement, and byte-identical
sources. They also locate two asymmetric colored PDF markers in real rendered
PNGs for every rotation, inherited/cropped/shifted boxes, 72/144/150 DPI, and
width-based raster rounding. The existing `image` crate is added only as a core
dev dependency to decode those PNGs; runtime dependencies and native pins do not
change.

Unusual malformed page trees remain uncharacterized. The current bundled build
has no XFA support. This milestone establishes a read-only snapshot and conversion
contract only; editing and Phase 2A.2 remain future work.

### Observed `/UserUnit` behavior in the bundled runtime

Controlled native tests characterize PDFium `151.0.7881.0`, Windows x64, with
`pdfium-render 0.9.4` and the `pdfium_7881` ABI. The tested DLL's SHA-256 is
`79d4676b656cfb1abcea88f9ade3b4b0826c5200382db5f4ec72a636c598c118`, matching
`third_party/pdfium/README.md`. This is an observation of that bundled binary,
not an assumption about another PDFium build or PDF viewer.

`editor_user_unit_1.pdf` and `editor_user_unit_2.pdf` differ only in four explicit
page-dictionary `/UserUnit` values. Both contain identical MediaBox
`[-50,-40,350,260]`, CropBox `[25,30,225,180]`, unmodified content streams, and
pages rotated 0/90/180/270. Tests verify the input bytes become identical after
replacing `/UserUnit 1` with `/UserUnit 2`, so geometry/content differences cannot
confound the comparison. Blue/red square centers are source coordinates `(75,65)`
and `(175,135)`; each square spans 10 source coordinate units per side.

**The currently bundled PDFium ignores `/UserUnit` in both geometry inspection
and the tested rendering paths.** The two files report exactly equal declared
boxes, visible boxes, rotations, native display dimensions, and document-info
results. At 0/180 degrees, the reported visible dimensions are 200 x 150; at
90/270, they are 150 x 200. The visible box stays `[25,30,225,180]` for both values.

| Rendering request | 0/180-degree raster | 90/270-degree raster | UserUnit 1 vs 2 |
| --- | --- | --- | --- |
| 72 DPI | 200 x 150 | 150 x 200 | Identical dimensions and decoded pixels |
| 144 DPI | 400 x 300 | 300 x 400 | Identical dimensions and decoded pixels |
| 150 DPI | 417 x 313 | 313 x 417 | Identical dimensions and decoded pixels |
| Target width 401 | 401 x 301 | 401 x 535 | Identical dimensions and decoded pixels |

The blue source marker `(75,65)` appears at these independently expected bitmap
locations at 72 DPI for both files, relative to the bitmap's top-left corner:

| Intrinsic rotation | Blue marker center |
| --- | --- |
| 0 | `(50,115)` |
| 90 clockwise | `(35,50)` |
| 180 | `(150,35)` |
| 270 clockwise | `(115,150)` |

The existing, unchanged `PageTransform` maps both source markers to the rendered
colors for both files, all four rotations, 72/144/150 DPI, and width-based raster
rounding. Tests also check the scale constructor at scales 1 and 2 against the
independent blue-marker locations, shifted display origins, inverse round trips,
and the native raster dimensions at 72/144 DPI. All source bytes remain unchanged.
Geometry and rendering are therefore internally consistent for these fixtures;
no editor/render misalignment or normalization fix is needed for this build.

This establishes alignment with PDFium's effective convention, **not faithful
physical-size support for non-default `/UserUnit`**. With `/UserUnit = 2`, the
engine still treats the source numeric coordinates and dimensions as if the
value were 1. There is no geometry-only multiplication by `/UserUnit`: changing
width/height alone would not establish correct box, marker, content, and rendering
semantics together. Any future support must normalize inspection, coordinates,
and rendering consistently inside PDF infrastructure. Flutter and individual
editor tools must continue to use the same engine-neutral `PageTransform`.

The focused tests intentionally pin this observed behavior and must be revisited
when the native runtime changes. Other `/UserUnit` values, unusual documents,
and physical-scale correction are outside this characterization. No broader
PDFium workaround or Phase 2A.2 implementation is introduced.

## Structural PDF engine

PDFium remains the rendering, preview, page-thumbnail, visual verification, and
existing Image-to-PDF engine. Content-preserving structural transformations are
separate. The application depends on the small `StructuralPdfEngine` capability
contract, whose Phase 1B.1 operations are runtime probing, structural validation,
and an internal content-preserving rewrite. The current implementation is
`QpdfCliEngine`:

```text
Flutter presentation
        |
typed flutter_rust_bridge boundary
        |
Rust application/core -> StructuralPdfEngine
                              |
                         QpdfCliEngine
                              |
                       QpdfProcessRunner
                              |
                      QpdfRuntimeResolver
                              |
                    bundled qpdf executable
```

The contract uses `Path`/`PathBuf`, typed version and result models, and stable
domain errors. It has no executable names, command switches, exit codes, process
handles, stdout, stderr, shell quoting, or Windows layout concepts. Core tests use
a small fake engine for orchestration and publication decisions. The qpdf crate
has real-runtime tests, while the application-level rewrite test coordinates qpdf
with PDFium to reopen the output, compare page counts, and render a representative
page.

`QpdfCliEngine` owns qpdf command semantics and interpretation of qpdf's success,
warning, invalid-document, and failure exits. `QpdfProcessRunner` is the single
owner of `Command` setup: it launches the resolved executable directly, passes
each path as a separate OS argument, never invokes a shell, drains stdout and
stderr concurrently, and retains at most 32 KiB from each diagnostic stream.
User-facing errors contain only stable messages. The runner does not log command
lines, document paths, or diagnostics. qpdf executable/helper override environment
variables are removed. The process is contained inside infrastructure, leaving a
future runner free to retain and terminate a child for cancellation without
exposing process objects to core.

`QpdfRuntimeResolver` locates only the controlled relocatable layout relative to
the application executable:

```text
ilikepdf.exe
pdfium.dll
data/
runtime/qpdf/
    runtime-manifest.txt
    README.md
    bin/
        qpdf.exe
        qpdf30.dll
        required MSVC runtime DLLs
    licenses/
        LICENSE.txt
        NOTICE.md
    provenance/
        qpdf-12.4.1.sha256
        qpdf-12.4.1.sha256.sigstore
```

There is deliberately no system `PATH` fallback. `QpdfCliEngine` is a desktop
implementation, not the application contract. A future platform where process
spawning is unsuitable may supply another `StructuralPdfEngine` implementation,
possibly backed by libqpdf FFI, without changing application workflows. No such
platform or FFI implementation is currently supported or included.

The current runtime is the official qpdf 12.4.1 Windows x86_64 MSVC archive,
`qpdf-12.4.1-msvc64.zip`, whose SHA-256 is
`3cd016cd433ef7232e42f4c13348a49cc14907a3c7278ef4f99120593126f7a6`.
`third_party/qpdf/runtime-manifest.txt` is the canonical machine-readable source
for its version, upstream artifact, checksums, executable, required runtime
files, license material, and provenance files. The adjacent README records the
upstream source and verification history. Windows CMake parses the manifest,
hashes every listed vendored file during configuration, fails on missing or
changed assets, and installs the self-contained layout automatically. The exact
upstream checksum manifest and Sigstore bundle are packaged for audit; no runtime
download or expensive launch-time hashing is performed.

Protect and Unlock keep passwords inside the application and qpdf infrastructure
boundary. The qpdf runner launches a child whose only public argument is `@-`;
the complete qpdf argument list, including any user password and generated owner
password, is delivered through standard input. This avoids process-command-line
exposure without creating a password file. Secret-bearing Rust values redact
their debug output and clear owned byte buffers on drop where practical. Bridge
password request types deliberately expose no debug or string representation,
and logging remains limited to allow-listed event names.

The rewrite spike never targets the source. Core creates a private destination-
directory working path, closes its initial handle so the external process can
write on Windows, asks the engine for a content-preserving rewrite, checks a
non-empty regular file, reopens and renders it with PDFium, verifies page-count
parity, syncs it, and atomically publishes without clobbering. Dropping the
pending output cleans up unsuccessful working files.

Merge PDF builds on the same boundary. The focused core workflow requires at
least two ordered input instances, deliberately preserves duplicate paths, and
uses the card order as the qpdf page-composition order while retaining page
order inside every source. Every page is copied structurally; the workflow does
not rasterize, resize, rotate, crop, or otherwise normalize mixed page geometry.
It preflights each instance for filesystem availability, qpdf validity and
password requirements, and PDFium page count. Recoverable qpdf warnings may
continue, but any fatal input failure stops the entire job without partial
publication.

The default Merge output name is `merged.pdf`. The editable field accepts only
a filename, appends `.pdf` when omitted, and cannot contain a directory or
Windows-reserved filename. The initial destination is captured from the first
PDF that establishes a new session; add, remove, and reorder operations do not
change it, and an explicitly selected destination persists until the workspace
is fully reset. Publication uses the lowest case-insensitive available name
(`merged.pdf`, `merged (1).pdf`, and so on) with an atomic no-clobber retry when
another writer wins a race.

qpdf first writes a private working file in the destination directory. Core then
requires structural validation, a successful PDFium reopen, and exact equality
between the output page count and the sum of all input-instance page counts.
Only then is the file published. Progress is stage-based (`Preparing`,
`Merging`, `Validating`, `Publishing`, `Completed`) because qpdf does not expose
deterministic per-page merge progress. Password entry is intentionally absent;
an input that requires a password is rejected with a typed instruction to
unlock it first.

Merge v1 makes no promise to intelligently combine every document-level feature
such as outlines, metadata, named destinations, AcroForm dictionaries, embedded
files, JavaScript, or document actions. It relies on qpdf's normal page
composition semantics and does not flatten source pages merely to avoid those
limitations.

Split PDF is a deliberately single-source structural workflow. It accepts
exactly one PDF with at least two pages and never anticipates batch state. The
source remains read-only, every source page appears exactly once in its original
order, and mixed page dimensions and orientation are preserved without
rasterization, quality controls, compression, or geometry normalization.

The application owns all four product policies: every page, every N pages,
strictly ascending comma-separated split points, and maximum generated file
size. These policies resolve to inclusive, ordered, contiguous page ranges. The
engine contract knows only how to create a private PDF from one such range:

```text
SplitPdf workflow policy
        |
StructuralPdfEngine::create_page_range
        |
QpdfCliEngine
        |
shared QpdfProcessRunner
```

Split's visual workspace obtains typed inclusive ranges through
`preview_split_pdf_ranges`, which calls the same pure core
`plan_split_pdf_ranges` policy as execution. Flutter owns raw editing text and
layout only. Each edit invalidates the prior preview and disables execution
until the current request succeeds; revision checks discard stale responses.
Invalid nonempty split-point text must be corrected before boundary clicks can
edit it. An empty field allows starting a boundary selection. Final-page
boundaries remain disabled, while page 1 is valid.

The workspace virtualizes fixed-height rows inside output parts as well as
between parts. A prefix row index maps the visible rows to typed ranges;
continued rows repeat the part label and share a dashed outline. Every-page
outputs use compact per-card labels. A small independent `PageThumbnailCache`
uses PDFium previews at width 360, two concurrent renders, viewport-demanded
queues, and 32 retained entries (including failures). Source replacement waits
for prior in-flight renders, evictions remove temporary files and decoded-image
cache entries, and disposal discards late results. Group changes reuse page
identity and cached previews. No Organize workflow state is shared or changed.
Size mode shows ungrouped pages until execution supplies actual ranges and
sizes; preview never generates candidate PDFs.

In particular, size grouping is not a qpdf engine capability. The application
uses a deterministic greedy search and accepts or rejects each candidate from
the actual generated PDF byte size. The UI uses decimal megabytes (1 MB =
1,000,000 bytes), performs checked integer conversion, and does not run candidate
generation while settings are edited. Final part count and ranges are discovered
during execution; the workflow does not claim a globally minimal part count.
When a page alone is over the hard limit, Split fails without compression or
partial publication.

Every-page outputs use `<stem>-page-0001.pdf`; all other modes use
`<stem>-part-0001.pdf`, with numbering that expands beyond four digits. Parts are
retained in a same-filesystem private directory, then every part is structurally
validated, reopened with PDFium, checked for its expected page count, and, for
size mode, measured again against the hard byte limit. Only the complete
validated directory is renamed into `<stem>-split`; existing directories are
never merged or changed, and the lowest case-insensitive suffix such as
`<stem>-split (1)` is used on collision.

The default destination follows the currently selected source parent. Once the
user chooses a custom destination it survives mode changes and valid source
replacement; a fully cleared workspace resets that session state. A replacement
is inspected before it replaces the existing source. Password-protected input is
blocked without password UI. Preview uses PDFium page 1, but preview failure does
not invalidate an otherwise structurally usable source. Progress reports real
stages only: preparing, finding split points when needed, creating parts,
validating, publishing, and completed.

Organize PDF is one multi-source session that creates one output PDF; it is not
a batch workflow. Core owns the source set, stable page-item/page-plan model,
duplicate-source policy, initial flattening order, page deletion and rotation
semantics, reset behavior, output naming, and destination rules:

```text
Organize session and ordered page-plan policy
        |
StructuralPdfEngine::create_page_plan
        |
QpdfCliEngine
        |
shared QpdfProcessRunner
```

Sources are flattened in file insertion order and source page order. Adding
valid files later appends their pages to the current workspace and does not
disturb prior edits. Candidate additions are atomic as a group: if any candidate
is invalid, password-protected, or equivalent to an already-loaded path, none of
that candidate group is added and the existing session remains intact. Removing
a source removes all of its current and resettable page items. Deleting every
page does not remove its source entry; at least one remaining page is required
to execute. Reset restores the original order and zero rotation for every page
in the current source set, without re-reading source metadata or resurrecting a
source that was explicitly removed.

The engine receives only an arbitrary ordered list of source path, one-based
source page, and relative quarter-turn rotation. `QpdfCliEngine` maps that typed
plan to qpdf page composition and rotation arguments. It does not know session
IDs, deletion, reset, insertion order, or destination policy. Every invocation
uses the central runner, so direct executable launch, bounded diagnostics,
no-shell behavior, controlled runtime resolution, and the Windows hidden-console
flag remain shared with Merge and Split.

Organize defaults to `organized.pdf` and the first-added source's parent
directory. The filename field is filename-only and uses the existing Windows-safe
normalization. Adding/removing sources, editing pages, and Reset do not change an
established destination; an explicit custom destination persists until the
session becomes empty. Publication uses the same case-insensitive lowest-gap
numbering and atomic no-clobber persistence as Merge. Core preflights the complete
loaded source set at execution time, writes one private file, structurally
validates it, reopens it with PDFium, checks exact page-count equality, and only
then publishes. Any failure removes the private output and leaves every source
byte-identical.

Page thumbnails remain a presentation/preview concern and use PDFium, never the
structural output path. The Organize grid is lazily built near the viewport,
allows at most two native thumbnail renders concurrently, and keeps at most 32
rendered thumbnails in its own least-recently-used cache. Evicted and
session-owned temporary thumbnail files are removed on a best-effort basis.
This bounds eager work
and retained page bitmaps for large multi-file sessions while leaving room for a
future shared thumbnail scheduler if more page-centric tools need one.

Protect PDF and Unlock PDF are single-source structural workflows:

```text
Protect/Unlock presentation
        |
typed bridge request with no secret string representation
        |
core security workflow -> StructuralPdfEngine
                              |
                         QpdfCliEngine
                              |
                    sensitive stdin argument channel
```

Encryption inspection has three typed states: unencrypted, encrypted but
openable without a password, and encrypted with a password required. Protect
accepts only the first state and rejects empty, mismatched, multiline, or NUL-
containing passwords before execution. It uses qpdf AES-256 encryption with the
user password plus a fresh 256-bit cryptographically random owner password. The
owner password is internal and is never returned to Flutter or persisted.

Unlock treats an unencrypted source as an already-unlocked invalid request,
decrypts an encrypted source with no open password directly, and requires a
password only for the password-required state. A wrong password maps to the
typed `IncorrectPassword` error and publishes nothing. The UI keeps that value
only so the user can correct and retry it; successful execution, clearing the
workspace, and widget disposal clear the password controllers.

Both operations keep the source read-only and write to a private same-filesystem
output. Protect requires the private file to inspect as password-required, pass
qpdf's password-aware validation, reopen in PDFium with the password, and retain
the source page count. Unlock requires the private file to inspect as
unencrypted, pass qpdf validation, reopen in PDFium without a password, and
retain the source page count. Only then does core flush and atomically publish a
case-insensitive, non-clobbering output name. The defaults are
`<source>-protected.pdf` and `<source>-unlocked.pdf`; an explicit destination
persists across valid source replacement within the current session.

Encryption changes can invalidate existing digital signatures, and qpdf's
structural rewrite does not promise preservation of every document-level PDF
feature. Protect/Unlock do not offer permission presets, certificate security,
password recovery, or in-place replacement. Future secret-bearing operations
must reuse the sensitive runner path rather than adding command-line or
temporary-file password transport.

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

Merge PDF reuses the same workspace, drop target, card grid, page-1 PDFium
thumbnail path, destination picker, and anchored action. Each card has its own
presentation identity rather than using its path as identity, so the same file
may appear more than once and each instance can be reordered or removed
independently. A thumbnail failure remains a lightweight card state and does not
replace execution-time structural preflight.

Organize PDF reuses the application shell, file drop target, destination picker,
settings inspector, anchored action, and reorder interaction language. Its main
area is a lazy page-item grid rather than a document-card list. The right-side
inspector owns the insertion-ordered source list, whole-source removal, Reset all,
output filename, and destination. The page cards expose reorder, rotate-left,
rotate-right, and delete controls directly; dragging near a workspace edge scrolls
the lazy grid for long-distance moves. The displayed thumbnail is wrapped in the
current quarter-turn so presentation matches the structural output plan.
