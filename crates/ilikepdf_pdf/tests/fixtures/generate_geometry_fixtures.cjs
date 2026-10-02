// Deterministic, project-owned PDF fixtures; requires only Node's standard library.
// Run: node crates/ilikepdf_pdf/tests/fixtures/generate_geometry_fixtures.cjs
const fs = require('node:fs');
const path = require('node:path');

function writePdf(name, objects) {
  let body = '%PDF-1.7\n';
  const offsets = [0];
  objects.forEach((object, index) => {
    offsets.push(Buffer.byteLength(body, 'ascii'));
    body += `${index + 1} 0 obj\n${object}\nendobj\n`;
  });
  const xref = Buffer.byteLength(body, 'ascii');
  body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  body += offsets.slice(1).map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('');
  body += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  fs.writeFileSync(path.join(__dirname, name), body, 'ascii');
}

// Blue marker center = (75, 65), red marker center = (175, 135).
// Distinct asymmetric locations detect reflection, origin, and rotation mistakes.
const content = '0 0 1 rg 70 60 10 10 re f\n1 0 0 rg 170 130 10 10 re f\n';
const stream = `<< /Length ${Buffer.byteLength(content)} >>\nstream\n${content}endstream`;
const pages = [
  '/MediaBox [0 0 595.2756 841.8898]', // A4, no crop.
  '/MediaBox [0 0 612 792]', // Letter, no crop.
  ...[0, 90, 180, 270].map(rotation =>
    `/MediaBox [-50 -40 350 260] /CropBox [25 30 225 180] /Rotate ${rotation}`),
  '/MediaBox [0 0 300 200] /CropBox [-20 20 250 240]', // Partial intersection.
  '/MediaBox [-100 -200 200 300]', // Non-zero media origin, no crop.
  '', // Parent provides both boxes and 90-degree rotation.
  '', // Missing MediaBox: PDFium fallback Letter page.
  '/MediaBox [300 200 0 0] /CropBox [240 170 20 30]', // Reversed bounds normalize.
  '/MediaBox [0 0 300 200] /CropBox [0 0 0 0]', // Empty crop defaults to media.
];
const pageRefs = pages.map((_, index) => `${index + 4} 0 R`);
const inheritedNodeId = pages.length + 4;
const objects = [
  '<< /Type /Catalog /Pages 2 0 R >>',
  `<< /Type /Pages /Count ${pages.length} /Kids [${pageRefs.slice(0, 8).join(' ')} ${inheritedNodeId} 0 R ${pageRefs.slice(9).join(' ')}] >>`,
  stream,
  ...pages.map((boxes, index) =>
    `<< /Type /Page /Parent ${index === 8 ? inheritedNodeId : 2} 0 R /Resources << >> /Contents 3 0 R ${boxes} >>`),
  `<< /Type /Pages /Parent 2 0 R /Count 1 /Kids [${pageRefs[8]}] /MediaBox [-50 -40 350 260] /CropBox [25 30 225 180] /Rotate 90 >>`,
];
writePdf('editor_geometry.pdf', objects);
writePdf('editor_disjoint_boxes.pdf', [
  '<< /Type /Catalog /Pages 2 0 R >>',
  '<< /Type /Pages /Count 1 /Kids [4 0 R] >>',
  stream,
  '<< /Type /Page /Parent 2 0 R /Resources << >> /Contents 3 0 R /MediaBox [0 0 100 100] /CropBox [200 200 300 300] >>',
]);
writePdf('editor_empty.pdf', [
  '<< /Type /Catalog /Pages 2 0 R >>',
  '<< /Type /Pages /Count 0 /Kids [] >>',
]);

// These paired files differ only in the four /UserUnit numeric values. Keep
// identical non-zero boxes, content streams, and rotations to isolate UserUnit.
for (const userUnit of [1, 2]) {
  writePdf(`editor_user_unit_${userUnit}.pdf`, [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Count 4 /Kids [4 0 R 5 0 R 6 0 R 7 0 R] >>',
    stream,
    ...[0, 90, 180, 270].map(rotation =>
      `<< /Type /Page /Parent 2 0 R /Resources << >> /Contents 3 0 R /MediaBox [-50 -40 350 260] /CropBox [25 30 225 180] /Rotate ${rotation} /UserUnit ${userUnit} >>`),
  ]);
}
