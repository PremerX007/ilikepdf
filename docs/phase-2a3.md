# Phase 2A.3 completion report

Implemented the internal editor object model and interaction foundation. Source
PDFs remain read-only. There is no Text Box, real PDF content tool, writer,
save/export operation or Phase 2B implementation. No commit or push was made.

## Changed modules

- Core: `crates/ilikepdf_core/src/application/editor/objects.rs` owns objects,
  rectangles, IDs, selection and command history; `interaction.rs` owns projection,
  hit testing and gesture transactions; `objects/tests.rs` tests both. `editor.rs`
  and `lib.rs` expose the domain façade. `page_transform.rs` adds the authoritative
  inverse vector conversion for captured pointer deltas.
- Bridge: `crates/ilikepdf_bridge/src/api/editor.rs` adds opaque edit-state/gesture
  bindings, canonical snapshots and projected display DTOs. Rust and Dart bridge
  files were regenerated with `flutter_rust_bridge_codegen generate` (dependency
  checking/upgrading disabled for the offline installed environment).
- Flutter: `editor_interaction.dart` owns captured-pointer/transient preview
  state; `editor_object_overlay.dart` draws objects and selection decoration;
  `editor_viewport.dart` wires pointers, keyboard focus and the internal toolbar;
  `editor_panel.dart` creates/discards edit states on session replacement/close.
  These files live under `apps/desktop/lib/src/app/editor/`.
- Tests: `apps/desktop/test/editor_object_test.dart`,
  `editor_object_scenarios.dart`, updates to `editor_panel_test.dart`, and
  `apps/desktop/integration_test/editor_objects_test.dart`.
- Documentation/CI: `docs/architecture.md`, stable editor rules in `AGENTS.md`,
  and the object integration suite in `.github/workflows/ci.yml`.
- No dependency, native runtime, license, binding ABI pin or lockfile changed.

## Model and invariants

`EditorObject` has a stable, session-scoped typed ID, exactly one zero-based
source page index, `PrototypeRectangle` kind and `EditorRectangle`. Its private
fields can only be constructed through validated core state operations.
Rectangles use finite canonical unrotated PDF/source-point bounds
`left/bottom/right/top`, with positive finite width/height. Coordinates preserve
CropBox offsets and the source PDF origin. Objects store no zoom, scroll, DPR,
viewport or raster values.

The visible `PageGeometry` box is the boundary authority. Move clamps translation
while preserving size/page. Resize clamps only active canonical edges, fixes
opposite edges and enforces a minimum 4-source-point size per axis (the complete
page axis is the minimum when smaller than 4). Handles cannot invert rectangles.

## Overlay and interaction

Each existing page stack contains its PDF raster and `EditorObjectOverlay`,
using the exact `EditorPageLayout` rectangle, geometry and transform. Core returns
projected object rectangles and handle centers; Dart only subtracts the local
page-stack origin. Zoom/scroll/DPR/raster replacement cannot mutate objects.

Per-page vector order is drawing order; reverse-order canonical body hit testing
selects the topmost object. Selection is single-object and outside history.
Empty page/gap/background clicks clear it. Core prioritizes selected resize
handles, chooses the nearest when targets overlap and preserves a central body
target at low zoom. Eight handles are drawn at a constant 8 logical units, with
12-unit hit targets, independent of page zoom.

One gesture freezes session ID, revision, object/page ID, before-geometry,
`PageTransform`, pointer origin and operation kind. Every pointer frame invokes
only pure synchronous core geometry through FFI, with no native document call,
object/history mutation or document snapshot. Flutter retains one transient
display preview. Pointer-up commits one move/resize command after stale-state
validation. An 80-event drag is one edit. No-op/cancelled/stale gestures are not
edits. Escape, focus loss, scrolling, zoom, relayout and session replacement
discard incomplete gestures. Cross-page dragging is unavailable.

Resize handle canonical edges are resolved through inverse `PageTransform`,
without Flutter rotation branches. The same controls work at 0/90/180/270 degrees,
with positive/negative crop origins and mixed page dimensions.

## Commands, history and focus

`EditorCommand` has AddObject, MoveObject, ResizeObject and DeleteObject variants.
Move/resize store ID and before/after rectangles; add/delete store the object
and its stacking position. Undo/redo preserve identity, page, geometry and order.
Removing selection clears it; undo restores the object without automatically
reselecting it. New effective edits discard redo; selection and no-op gestures
do not. History retains **100 commands total across both branches**, and there
are at most **1,000 live objects**. Oldest history entries are dropped at capacity.

The existing internal entry remains gated by
`--dart-define=ILIKEPDF_EDITOR_VIEWPORT=true`. Its toolbar provides Add test
object, Undo, Redo and Delete. Delete, Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z operate
only while the workspace's own focus node is primary; other/future text inputs
own their editing keys. Toolbar editing actions activate that editor context.

Successful opening/replacement creates a clean state and drops all prior
objects, selection, previews and history. Failed replacement preserves the
valid session, including its selection and undo/redo branches. Late renders and
pointer-up events from replaced sessions are rejected.

## Verification

- Rust: formatting check, Clippy with warnings denied, and workspace/all-target
  tests passed: **177 tests**, plus the separately exercised release-runtime test.
  Eight new core tests cover validation, insertion, per-page ownership, stable
  IDs, stacking, deletion/restoration, sequential commands, redo branching,
  history/object caps, gesture transaction counts and stale sessions.
- Coordinate tests cover 25/64/100/125/200/250/400-percent scales across the new
  and existing suites, scrolled layouts, mixed sizes, crop offsets, all rotations,
  all eight resize handles, opposite-edge stability, page/minimum clamps and
  fractional source geometry. Canonical values remain stable across zoom,
  scrolling and raster-size changes, with source-point tolerance of 1e-7.
- Dart formatting validation and Flutter analysis passed. Full Flutter unit/
  widget suite: **118 passed**. Semantic/state assertions cover projected overlay
  placement, constant handles and transient commit/cancel behavior.
- Windows editor objects: **6 passed** with real Rust/bridge/PDFium, including
  80-frame move/resize transactions, deletion/history shortcuts, topmost
  selection, focus ownership, failed/successful replacement, late results and
  exact 100% → 200% → 400% → 100% projection/raster checks.
- Existing Windows viewport: **4 passed**, including lazy/bounded rendering,
  64-page scrolling, mixed sizes, rotations, fit width, zoom center anchors,
  stale-result protection and DPR-two sharp rasters through 400%.
- Existing Windows suites passed individually: app info (1), PDF preview (6),
  image-to-PDF (3), merge (2), split (3), organize (1), PDF security (1).
  A combined multi-target runner lost its debug connection; individual fresh
  processes passed. A shared native-asset cache failure during concurrent debug/
  release builds was resolved by cleaning generated metadata and running builds
  sequentially; dependencies stayed unchanged.
- `git diff --check` passed. Final sequential `flutter build windows --release`
  passed. Required release files/notices/provenance are present; the bundled
  runtime test loads PDFium and resolves/probes qpdf 12.4.1 without PATH fallback.
  Packaged PDFium SHA-256 matches the pinned
  `79d4676b656cfb1abcea88f9ade3b4b0826c5200382db5f4ec72a636c598c118`.
- Manual human verification was not performed; the Windows interaction results
  above are automated.

Remaining limitations are intentional: prototype rectangles only, one selected
object, no cross-page move, rotation, aspect lock, snapping, guides, persistence
or export. Incomplete gestures are cancelled when the viewport changes. Very
small objects have overlapping handle decoration at low zoom; nearest-handle
hit testing and the central body target keep behavior deterministic.
