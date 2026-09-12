import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

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
  Stream<PdfBatchExportUpdate> exportBatch({
    required List<String> sourcePaths,
    required PdfDestinationMode destinationMode,
    required String? customDestinationDirectory,
    required PdfImageQuality quality,
    required PdfImageFormat format,
  }) async* {
    await for (final update in const LocalPdfToImageWorkflow().exportBatch(
      sourcePaths: sourcePaths,
      destinationMode: destinationMode,
      customDestinationDirectory: customDestinationDirectory,
      quality: quality,
      format: format,
    )) {
      if (update.status == PdfBatchExportStatus.complete) {
        exportedOutputs = update.documents
            .expand((document) => document.outputFiles)
            .toList(growable: false);
      }
      yield update;
    }
  }

  @override
  Future<List<SelectedPdf>> selectPdfs() async {
    final info = await rust.openPdfDocument(sourcePath: source.path);
    return [
      SelectedPdf(
        displayName: 'two_page.pdf',
        sourcePath: source.path,
        sourceDirectory: source.parent.path,
        pageCount: info.pageCount,
        firstPageWidthPoints: info.firstPageSize?.widthPoints,
        firstPageHeightPoints: info.firstPageSize?.heightPoints,
      ),
    ];
  }

  @override
  Future<List<SelectedPdf>> preparePdfPaths(List<String> sourcePaths) =>
      selectPdfs();

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

class BatchUiFixtureWorkflow implements PdfToImageWorkflow {
  BatchUiFixtureWorkflow(this.selections, this.destination);

  final List<List<File>> selections;
  final Directory destination;
  final List<File> renderedPreviews = [];
  PdfBatchExportUpdate? finalUpdate;

  @override
  Future<String?> chooseDestinationDirectory() async => destination.path;

  @override
  Stream<PdfBatchExportUpdate> exportBatch({
    required List<String> sourcePaths,
    required PdfDestinationMode destinationMode,
    required String? customDestinationDirectory,
    required PdfImageQuality quality,
    required PdfImageFormat format,
  }) async* {
    await for (final update in const LocalPdfToImageWorkflow().exportBatch(
      sourcePaths: sourcePaths,
      destinationMode: destinationMode,
      customDestinationDirectory: customDestinationDirectory,
      quality: quality,
      format: format,
    )) {
      if (update.status != PdfBatchExportStatus.running) {
        finalUpdate = update;
      }
      yield update;
    }
  }

  @override
  Future<List<SelectedPdf>> preparePdfPaths(List<String> sourcePaths) =>
      const LocalPdfToImageWorkflow().preparePdfPaths(sourcePaths);

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) async {
    final rendered = await const LocalPdfToImageWorkflow().renderFirstPage(
      sourcePath,
    );
    renderedPreviews.add(File(rendered.outputPath));
    return rendered;
  }

  @override
  Future<List<SelectedPdf>> selectPdfs() {
    final selected = selections.isEmpty ? <File>[] : selections.removeAt(0);
    return preparePdfPaths(
      selected.map((file) => file.path).toList(growable: false),
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
            format: rust_export.PdfExportFormat.png,
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

  testWidgets(
    'Flutter exports a mixed PDF batch with custom destination and collision numbering',
    (WidgetTester tester) async {
      final sources = await Directory.systemTemp.createTemp(
        'ilikepdf-batch-sources-',
      );
      final destination = await Directory.systemTemp.createTemp(
        'ilikepdf-batch-output-',
      );
      addTearDown(() async {
        if (await sources.exists()) {
          await sources.delete(recursive: true);
        }
        if (await destination.exists()) {
          await destination.delete(recursive: true);
        }
      });
      final cover = await File(
        '${sources.path}${Platform.pathSeparator}cover.pdf',
      ).writeAsBytes(await _fixture('one_page.pdf').readAsBytes());
      final broken = await File(
        '${sources.path}${Platform.pathSeparator}broken.pdf',
      ).writeAsBytes(await _fixture('malformed.pdf').readAsBytes());
      final report = await File(
        '${sources.path}${Platform.pathSeparator}report.pdf',
      ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());
      final before = {
        cover.path: await cover.readAsBytes(),
        broken.path: await broken.readAsBytes(),
        report.path: await report.readAsBytes(),
      };
      final prepared = await const LocalPdfToImageWorkflow().preparePdfPaths([
        cover.path,
        broken.path,
        report.path,
      ]);
      expect(prepared.map((pdf) => pdf.displayName), [
        'cover.pdf',
        'broken.pdf',
        'report.pdf',
      ]);
      expect(
        prepared[1].inspectionProblem?.code,
        PdfExportProblemCode.invalidPdf,
      );

      Future<List<rust_export.PdfBatchExportUpdate>> export() => rust_export
          .exportPdfBatchToImages(
            request: rust_export.ExportPdfBatchRequest(
              sourcePaths: [cover.path, broken.path, report.path],
              destinationMode: rust_export.PdfBatchDestinationMode.customFolder,
              customDestinationDirectory: destination.path,
              quality: rust_export.PdfExportQuality.highQuality,
              format: rust_export.PdfExportFormat.png,
            ),
          )
          .toList();

      final firstUpdates = await export();
      final first = firstUpdates.last;
      expect(
        firstUpdates.first.status,
        rust_export.PdfBatchExportStatus.running,
      );
      expect(first.status, rust_export.PdfBatchExportStatus.completeWithErrors);
      expect(first.totalDocumentCount, 3);
      expect(first.succeededDocumentCount, 2);
      expect(first.failedDocumentCount, 1);
      expect(first.totalPageCount, 3);
      expect(first.completedPageCount, 3);
      expect(first.documents.map((document) => document.displayName), [
        'cover.pdf',
        'broken.pdf',
        'report.pdf',
      ]);
      expect(first.documents[1].error?.code.name, 'invalidPdf');
      expect(
        File(first.documents[0].outputFiles.single).uri.pathSegments.last,
        'cover-page-0001.png',
      );
      expect(
        File(first.documents[2].outputFiles.first).parent.path,
        '${destination.path}${Platform.pathSeparator}report',
      );
      expect(
        first.documents[2].outputFiles.map(
          (path) => File(path).uri.pathSegments.last,
        ),
        ['report-page-0001.png', 'report-page-0002.png'],
      );
      expect(
        await _pngDimensions(File(first.documents[0].outputFiles.single)),
        (1250, 833),
      );
      expect(await _pngDimensions(File(first.documents[2].outputFiles[1])), (
        833,
        1250,
      ));
      final firstCoverBytes = await File(first.documents[0].outputFiles.single)
          .readAsBytes();

      final second = (await export()).last;
      expect(
        File(second.documents[0].outputFiles.single).uri.pathSegments.last,
        'cover-page-0001 (1).png',
      );
      expect(
        File(second.documents[2].outputFiles.first).parent.path,
        '${destination.path}${Platform.pathSeparator}report (1)',
      );
      expect(
        await File(first.documents[0].outputFiles.single).readAsBytes(),
        firstCoverBytes,
      );
      for (final source in [cover, broken, report]) {
        expect(await source.readAsBytes(), before[source.path]);
      }
    },
  );

  testWidgets('batch next-to-source mode uses each PDF directory', (
    WidgetTester tester,
  ) async {
    final firstDirectory = await Directory.systemTemp.createTemp(
      'ilikepdf-next-source-a-',
    );
    final secondDirectory = await Directory.systemTemp.createTemp(
      'ilikepdf-next-source-b-',
    );
    addTearDown(() async {
      await firstDirectory.delete(recursive: true);
      await secondDirectory.delete(recursive: true);
    });
    final cover = await File(
      '${firstDirectory.path}${Platform.pathSeparator}ใบปก.pdf',
    ).writeAsBytes(await _fixture('one_page.pdf').readAsBytes());
    final report = await File(
      '${secondDirectory.path}${Platform.pathSeparator}report.pdf',
    ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());

    final completed =
        (await rust_export
                .exportPdfBatchToImages(
                  request: rust_export.ExportPdfBatchRequest(
                    sourcePaths: [cover.path, report.path],
                    destinationMode:
                        rust_export.PdfBatchDestinationMode.nextToSourceFiles,
                    customDestinationDirectory: null,
                    quality: rust_export.PdfExportQuality.standard,
                    format: rust_export.PdfExportFormat.png,
                  ),
                )
                .toList())
            .last;

    expect(completed.status, rust_export.PdfBatchExportStatus.complete);
    expect(completed.documents[0].outputFiles, [
      '${firstDirectory.path}${Platform.pathSeparator}ใบปก-page-0001.png',
    ]);
    expect(
      File(completed.documents[1].outputFiles.first).parent.path,
      '${secondDirectory.path}${Platform.pathSeparator}report',
    );
  });

  testWidgets('release workflow selects adds and exports multiple PDF cards', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1280, 800);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final sources = await Directory.systemTemp.createTemp(
      'ilikepdf-batch-ui-sources-',
    );
    final destination = await Directory.systemTemp.createTemp(
      'ilikepdf-batch-ui-output-',
    );
    final cover = await File(
      '${sources.path}${Platform.pathSeparator}cover.pdf',
    ).writeAsBytes(await _fixture('one_page.pdf').readAsBytes());
    final report = await File(
      '${sources.path}${Platform.pathSeparator}report.pdf',
    ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());
    final appendix = await File(
      '${sources.path}${Platform.pathSeparator}appendix.pdf',
    ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());
    final before = {
      for (final source in [cover, report, appendix])
        source.path: await source.readAsBytes(),
    };
    final workflow = BatchUiFixtureWorkflow([
      [cover, report],
      [appendix],
    ], destination);
    addTearDown(() async {
      for (final preview in workflow.renderedPreviews) {
        if (await preview.exists()) {
          await preview.delete();
        }
      }
      if (await sources.exists()) {
        await sources.delete(recursive: true);
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
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    expect(find.text('2 selected PDFs'), findsOneWidget);
    await tester.tap(find.text('Add PDFs'));
    await tester.pumpAndSettle();
    expect(find.text('3 selected PDFs'), findsOneWidget);

    await tester.ensureVisible(find.text('High'));
    await tester.tap(find.byKey(const ValueKey('quality-High')));
    await tester.tap(find.text('JPG'));
    await tester.tap(find.text('Custom folder'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Choose folder'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Convert to images'));
    await tester.pumpAndSettle();

    final completed = workflow.finalUpdate;
    expect(completed?.status, PdfBatchExportStatus.complete);
    expect(completed?.succeededDocumentCount, 3);
    expect(completed?.completedPageCount, 5);
    expect(find.text('3 PDFs completed · 0 PDFs failed'), findsOneWidget);
    expect(
      File(completed!.documents[0].outputFiles.single).uri.pathSegments.last,
      'cover-page-0001.jpg',
    );
    expect(
      await _imageDimensions(File(completed.documents[0].outputFiles.single)),
      (1250, 833),
    );
    expect(
      (await File(
        completed.documents[0].outputFiles.single,
      ).readAsBytes()).take(3),
      [255, 216, 255],
    );
    for (final source in [cover, report, appendix]) {
      expect(await source.readAsBytes(), before[source.path]);
    }
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
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    expect(find.text('PDF 1 · 2 pages'), findsOneWidget);
    expect(find.text('Next to source files'), findsOneWidget);
    expect(find.text('Preview page 1'), findsNothing);
    expect(find.byKey(const ValueKey('pdf-preview-image')), findsOneWidget);
    expect(find.byType(Image), findsOneWidget);

    await tester.ensureVisible(find.text('Custom folder'));
    await tester.tap(find.text('Custom folder'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Choose folder'));
    await tester.tap(find.text('Choose folder'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Convert to images'));
    await tester.tap(find.text('Convert to images'));
    await tester.pumpAndSettle();

    expect(find.text('Conversion complete'), findsOneWidget);
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

Future<(int, int)> _imageDimensions(File file) async {
  final codec = await ui.instantiateImageCodec(await file.readAsBytes());
  final frame = await codec.getNextFrame();
  final dimensions = (frame.image.width, frame.image.height);
  frame.image.dispose();
  codec.dispose();
  return dimensions;
}

File _fixture(String name) =>
    File('../../crates/ilikepdf_pdf/tests/fixtures/$name').absolute;
