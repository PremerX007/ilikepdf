import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/app.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';
import 'package:ilikepdf/src/rust/api/pdf_export.dart' as rust_export;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust;
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

class FixturePdfWorkflow implements PdfToImageWorkflow {
  FixturePdfWorkflow(this.source, this.destination);

  final File source;
  final Directory destination;
  File? renderedOutput;
  List<String> exportedOutputs = const [];

  @override
  Future<String?> chooseDestinationDirectory() async => destination.path;

  @override
  Stream<PdfImageExportUpdate> exportAllPages({
    required String sourcePath,
    required String destinationDirectory,
    required PdfImageQuality quality,
  }) async* {
    await for (final update in const LocalPdfToImageWorkflow().exportAllPages(
      sourcePath: sourcePath,
      destinationDirectory: destinationDirectory,
      quality: quality,
    )) {
      if (update.status == PdfImageExportStatus.complete) {
        exportedOutputs = update.outputFiles;
      }
      yield update;
    }
  }

  @override
  Future<SelectedPdf?> selectAndInspect() async {
    final info = await rust.openPdfDocument(sourcePath: source.path);
    return SelectedPdf(
      displayName: 'two_page.pdf',
      sourcePath: source.path,
      sourceDirectory: source.parent.path,
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

  testWidgets('Flutter receives progress while Rust exports all pages', (
    WidgetTester tester,
  ) async {
    final source = File('../../crates/ilikepdf_pdf/tests/fixtures/two_page.pdf')
        .absolute;
    final before = await source.readAsBytes();
    final destination = await Directory.systemTemp.createTemp(
      'ilikepdf-export-integration-',
    );
    addTearDown(() => destination.delete(recursive: true));

    final updates = await rust_export
        .exportPdfToImages(
          request: rust_export.ExportPdfToImagesRequest(
            sourcePath: source.path,
            destinationDirectory: destination.path,
            quality: rust_export.PdfExportQuality.standard,
          ),
        )
        .toList();

    final completed = updates.last;
    expect(updates.first.status, rust_export.PdfExportStatus.running);
    expect(completed.status, rust_export.PdfExportStatus.complete);
    expect(completed.totalPageCount, 2);
    expect(completed.completedPageCount, 2);
    expect(
      completed.outputFiles.map((path) => File(path).uri.pathSegments.last),
      ['two_page-page-0001.png', 'two_page-page-0002.png'],
    );
    expect(
      File(completed.outputFiles[0]).parent.path,
      '${destination.path}${Platform.pathSeparator}two_page',
    );
    expect(await _pngDimensions(File(completed.outputFiles[0])), (625, 417));
    expect(await _pngDimensions(File(completed.outputFiles[1])), (417, 625));
    expect(await source.readAsBytes(), before);
  });

  testWidgets('selection automatically shows a compact Rust preview', (
    WidgetTester tester,
  ) async {
    final source = File('../../crates/ilikepdf_pdf/tests/fixtures/two_page.pdf')
        .absolute;
    final before = await source.readAsBytes();
    final destination = await Directory.systemTemp.createTemp(
      'ilikepdf-export-ui-',
    );
    final workflow = FixturePdfWorkflow(source, destination);
    addTearDown(() async {
      final output = workflow.renderedOutput;
      if (output != null && await output.exists()) {
        await output.delete();
      }
      if (await destination.exists()) {
        await destination.delete(recursive: true);
      }
    });

    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'iLikePDF',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfToImageWorkflow: workflow,
      ),
    );
    await tester.tap(find.byKey(const ValueKey('tool-card-pdf-to-images')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
    expect(find.text('2 pages'), findsOneWidget);
    expect(find.text(source.parent.path), findsOneWidget);
    expect(find.text('Preview page 1'), findsNothing);
    expect(find.byKey(const ValueKey('pdf-preview-thumbnail')), findsOneWidget);
    expect(find.byKey(const ValueKey('pdf-preview-image')), findsOneWidget);
    expect(find.byType(Image), findsOneWidget);

    await tester.ensureVisible(find.text('Choose folder'));
    await tester.tap(find.text('Choose folder'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Convert to images'));
    await tester.tap(find.text('Convert to images'));
    await tester.pumpAndSettle();

    expect(find.text('Export complete: 2 PNG images created.'), findsOneWidget);
    expect(workflow.exportedOutputs.length, 2);
    expect(
      workflow.exportedOutputs.map((path) => File(path).uri.pathSegments.last),
      ['two_page-page-0001.png', 'two_page-page-0002.png'],
    );
    expect(await source.readAsBytes(), before);
  });
}

Future<(int, int)> _pngDimensions(File file) async {
  final bytes = await file.readAsBytes();
  final data = ByteData.sublistView(bytes);
  return (data.getUint32(16), data.getUint32(20));
}
