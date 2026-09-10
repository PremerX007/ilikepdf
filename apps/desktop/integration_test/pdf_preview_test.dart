import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/app.dart';
import 'package:ilikepdf/src/app/pdf_preview/pdf_preview_workflow.dart';
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust;
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

class FixturePdfWorkflow implements PdfPreviewWorkflow {
  FixturePdfWorkflow(this.source);

  final File source;
  File? renderedOutput;

  @override
  Future<SelectedPdf?> selectAndInspect() async {
    final info = await rust.openPdfDocument(sourcePath: source.path);
    return SelectedPdf(
      displayName: 'two_page.pdf',
      sourcePath: source.path,
      pageCount: info.pageCount,
      firstPageWidthPoints: info.firstPageSize?.widthPoints,
      firstPageHeightPoints: info.firstPageSize?.heightPoints,
    );
  }

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) async {
    final result = await rust.renderPdfPage(
      request: rust.RenderPdfPageRequest(
        sourcePath: sourcePath,
        pageIndex: 0,
        targetWidth: 600,
        destinationPath: null,
      ),
    );
    renderedOutput = File(result.outputPath);
    return RenderedPdfPage(
      outputPath: result.outputPath,
      widthPixels: result.widthPixels,
      heightPixels: result.heightPixels,
    );
  }
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets('Flutter opens and renders a PDF through the Rust core', (
    WidgetTester tester,
  ) async {
    final source = File('../../crates/ilikepdf_pdf/tests/fixtures/two_page.pdf')
        .absolute;
    final before = await source.readAsBytes();

    final info = await rust.openPdfDocument(sourcePath: source.path);
    final rendered = await rust.renderPdfPage(
      request: rust.RenderPdfPageRequest(
        sourcePath: source.path,
        pageIndex: 0,
        targetWidth: 600,
        destinationPath: null,
      ),
    );
    final png = File(rendered.outputPath);
    addTearDown(() async {
      if (await png.exists()) {
        await png.delete();
      }
    });

    expect(info.pageCount, 2);
    expect(rendered.widthPixels, 600);
    expect(rendered.heightPixels, 400);
    expect((await png.readAsBytes()).take(8), [
      137,
      80,
      78,
      71,
      13,
      10,
      26,
      10,
    ]);
    expect(await source.readAsBytes(), before);
  });

  testWidgets('preview UI displays the page rendered by Rust', (
    WidgetTester tester,
  ) async {
    final source = File('../../crates/ilikepdf_pdf/tests/fixtures/two_page.pdf')
        .absolute;
    final before = await source.readAsBytes();
    final workflow = FixturePdfWorkflow(source);
    addTearDown(() async {
      final output = workflow.renderedOutput;
      if (output != null && await output.exists()) {
        await output.delete();
      }
    });

    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'ilikepdf',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfWorkflow: workflow,
      ),
    );
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
    expect(find.text('2 pages'), findsOneWidget);

    await tester.tap(find.text('Render page 1'));
    await tester.pumpAndSettle();

    expect(find.text('Rendered preview'), findsOneWidget);
    expect(find.text('600 × 400 pixels'), findsOneWidget);
    expect(find.byType(Image), findsOneWidget);
    expect(await source.readAsBytes(), before);
  });
}
