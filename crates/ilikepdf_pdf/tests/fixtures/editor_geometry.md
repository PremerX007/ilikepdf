# Editor geometry fixtures

`editor_viewport_64.pdf` repeats A4 portrait, Letter landscape, a cropped custom
page, and that custom page rotated 90 degrees, for 64 pages. It uses the same
deterministic blue/red markers. Windows viewport tests use it to verify lazy
opening, incremental scrolling, bounded decoded-image retention, and repeated
zoom replacement. `editor_geometry.pdf` remains the four-rotation alignment oracle.

Regenerate with `node crates/ilikepdf_pdf/tests/fixtures/generate_geometry_fixtures.cjs`.
No network, private documents, PDF tooling, or third-party Node packages are used.
PDF streams, object IDs, page order, and xref offsets are deterministic.

`editor_geometry.pdf` has these zero-based pages:

| Page | MediaBox | CropBox | Rotation | Effective visible box |
| --- | --- | --- | --- | --- |
| 0 | 0,0,595.2756,841.8898 | absent | 0 | A4 MediaBox |
| 1 | 0,0,612,792 | absent | 0 | Letter MediaBox |
| 2–5 | -50,-40,350,260 | 25,30,225,180 | 0/90/180/270 | 25,30,225,180 |
| 6 | 0,0,300,200 | -20,20,250,240 | 0 | 0,20,250,200 |
| 7 | -100,-200,200,300 | absent | 0 | MediaBox |
| 8 | inherited | inherited | inherited 90 | 25,30,225,180 |
| 9 | absent | absent | 0 | native Letter fallback |
| 10 | reversed 300,200,0,0 | reversed 240,170,20,30 | 0 | 20,30,240,170 |
| 11 | 0,0,300,200 | empty 0,0,0,0 | 0 | MediaBox |

Every page has a blue square centered on PDF point `(75,65)` and a red square
centered on `(175,135)`, each 10 × 10 PDF points. Their asymmetric positions and
different colors provide an independent native-rendering oracle for transforms.
Box entries above are in unrotated source user space, with `/UserUnit = 1`.

`editor_disjoint_boxes.pdf` has non-intersecting media/crop boxes; usable geometry
inspection must fail. `editor_empty.pdf` has no pages; session creation must fail.
The Letter fallback and malformed-box fixtures characterize the pinned native
engine's behavior rather than claiming those inputs conform to the PDF standard.

## UserUnit comparison fixtures

`editor_user_unit_1.pdf` and `editor_user_unit_2.pdf` each have four pages, rotated
0/90/180/270 clockwise. Every page has the same MediaBox `[-50,-40,350,260]`,
CropBox `[25,30,225,180]`, and blue/red content markers described above. The only
file differences are the four numeric `/UserUnit` entries (1 versus 2); replacing
those same-length entries makes the complete file bytes identical, including
object IDs, streams, and xref offsets.

The bundled PDFium `151.0.7881.0` ignores those entries in geometry inspection and
rendering. Both files report a visible box `[25,30,225,180]` and display dimensions
200 x 150 (0/180) or 150 x 200 (90/270). Their decoded renders are pixel-identical
at 72/144/150 DPI and target width 401. At 72 DPI, the blue marker center is
`(50,115)`, `(35,50)`, `(150,35)`, `(115,150)` in bitmap coordinates for those four
rotations. Existing PageTransform alignment is verified for both source markers.
This pins the current engine behavior; it does not implement non-default UserUnit
physical-size scaling. See the observed UserUnit section in `docs/architecture.md`.
