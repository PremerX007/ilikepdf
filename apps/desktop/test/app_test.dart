import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/app.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';

class FakePdfToImageWorkflow implements PdfToImageWorkflow {
  FakePdfToImageWorkflow({
    required this.previewPath,
    this.failure,
    this.previewFails = false,
  });

  final String previewPath;
  final PdfExportProblem? failure;
  final bool previewFails;
  PdfImageQuality? exportedQuality;
  PdfImageFormat? exportedFormat;
  PdfDestinationMode? exportedDestinationMode;
  String? exportedCustomDestination;
  List<String> exportedSourcePaths = const [];
  int previewCalls = 0;
  SelectedPdf selectedPdf = const SelectedPdf(
    displayName: 'fixture.pdf',
    sourcePath: r'C:\fixtures\fixture.pdf',
    sourceDirectory: r'C:\fixtures',
    pageCount: 2,
    firstPageWidthPoints: 300,
    firstPageHeightPoints: 200,
  );

  @override
  Future<String?> chooseDestinationDirectory() async => r'C:\exports';

  @override
  Stream<PdfBatchExportUpdate> exportBatch({
    required List<String> sourcePaths,
    required PdfDestinationMode destinationMode,
    required String? customDestinationDirectory,
    required PdfImageQuality quality,
    required PdfImageFormat format,
  }) async* {
    exportedQuality = quality;
    exportedFormat = format;
    exportedDestinationMode = destinationMode;
    exportedCustomDestination = customDestinationDirectory;
    exportedSourcePaths = List.unmodifiable(sourcePaths);
    if (failure case final error?) {
      yield PdfBatchExportUpdate(
        status: PdfBatchExportStatus.failed,
        totalDocumentCount: sourcePaths.length,
        completedDocumentCount: 0,
        succeededDocumentCount: 0,
        failedDocumentCount: 0,
        totalPageCount: 2,
        completedPageCount: 0,
        currentDocumentIndex: 1,
        currentDocumentFilename: selectedPdf.displayName,
        currentPage: null,
        documents: const [],
        error: error,
      );
      return;
    }
    yield PdfBatchExportUpdate(
      status: PdfBatchExportStatus.running,
      totalDocumentCount: sourcePaths.length,
      completedDocumentCount: 0,
      succeededDocumentCount: 0,
      failedDocumentCount: 0,
      totalPageCount: 2,
      completedPageCount: 1,
      currentDocumentIndex: 1,
      currentDocumentFilename: selectedPdf.displayName,
      currentPage: 1,
      documents: const [],
      error: null,
    );
    yield PdfBatchExportUpdate(
      status: PdfBatchExportStatus.complete,
      totalDocumentCount: sourcePaths.length,
      completedDocumentCount: sourcePaths.length,
      succeededDocumentCount: sourcePaths.length,
      failedDocumentCount: 0,
      totalPageCount: 2,
      completedPageCount: 2,
      currentDocumentIndex: null,
      currentDocumentFilename: null,
      currentPage: null,
      documents: [
        PdfBatchDocumentOutcome(
          sourcePath: r'C:\fixtures\fixture.pdf',
          displayName: 'fixture.pdf',
          totalPageCount: 2,
          completedPageCount: 2,
          outputFiles: [
            r'C:\exports\fixture\fixture-page-0001.png',
            r'C:\exports\fixture\fixture-page-0002.png',
          ],
          error: null,
        ),
      ],
      error: null,
    );
  }

  @override
  Future<List<SelectedPdf>> selectPdfs() async => [selectedPdf];

  @override
  Future<List<SelectedPdf>> preparePdfPaths(List<String> sourcePaths) async => [
    selectedPdf,
  ];

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) async {
    previewCalls += 1;
    if (previewFails) {
      throw StateError('preview failed');
    }
    return RenderedPdfPage(
      outputPath: previewPath,
      widthPixels: 1,
      heightPixels: 1,
    );
  }
}

void main() {
  late Directory previewDirectory;
  late String previewPath;

  setUpAll(() async {
    previewDirectory = await Directory.systemTemp.createTemp(
      'ilikepdf-widget-preview-',
    );
    final preview = File('${previewDirectory.path}\\preview.png');
    await preview.writeAsBytes(
      base64Decode(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
      ),
    );
    previewPath = preview.path;
  });

  tearDownAll(() => previewDirectory.delete(recursive: true));

  testWidgets('shows an unobtrusive version without redundant local chrome', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'iLikePDF',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfToImageWorkflow: FakePdfToImageWorkflow(previewPath: previewPath),
      ),
    );

    expect(find.text('iLikePDF'), findsOneWidget);
    expect(find.byKey(const ValueKey('application-version')), findsOneWidget);
    expect(find.text('Version 0.1.0'), findsOneWidget);
    expect(find.textContaining('Files stay on this computer'), findsNothing);
    expect(find.textContaining('Local processing'), findsNothing);
    expect(find.text('PDF tools'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('tool-card-pdf-to-images')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('tool-card-images-to-pdf')),
      findsOneWidget,
    );
    expect(find.text('Coming soon'), findsNWidgets(5));
  });

  testWidgets('implemented cards navigate and back returns Home', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'iLikePDF',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfToImageWorkflow: FakePdfToImageWorkflow(previewPath: previewPath),
      ),
    );

    await tester.tap(find.byKey(const ValueKey('tool-card-images-to-pdf')));
    await tester.pumpAndSettle();
    expect(find.text('Images to PDF'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('tool-workspace-surface')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('back-home-button')));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('tool-card-pdf-to-images')));
    await tester.pumpAndSettle();
    expect(find.text('PDF to Images'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('back-home-button')));
    await tester.pumpAndSettle();
    expect(find.text('PDF tools'), findsOneWidget);
  });

  testWidgets('coming-soon cards cannot open unfinished tools', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'iLikePDF',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfToImageWorkflow: FakePdfToImageWorkflow(previewPath: previewPath),
      ),
    );

    await tester.tap(find.byKey(const ValueKey('tool-card-merge-pdf')));
    await tester.pumpAndSettle();

    expect(find.text('PDF tools'), findsOneWidget);
    expect(find.byKey(const ValueKey('back-home-button')), findsNothing);
  });

  testWidgets('tool workspace uses columns wide and stacks when narrow', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1280, 760);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'iLikePDF',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfToImageWorkflow: FakePdfToImageWorkflow(previewPath: previewPath),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('tool-card-images-to-pdf')));
    await tester.pumpAndSettle();

    final workspace = find.byKey(const ValueKey('tool-workspace-surface'));
    final settings = find.byKey(const ValueKey('tool-settings-panel'));
    expect(
      tester.getRect(workspace).right,
      lessThan(tester.getRect(settings).left),
    );

    tester.view.physicalSize = const Size(760, 900);
    await tester.pumpAndSettle();
    expect(
      tester.getRect(settings).top,
      greaterThan(tester.getRect(workspace).bottom),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('selection starts a compact preview with streamlined quality', (
    WidgetTester tester,
  ) async {
    final workflow = FakePdfToImageWorkflow(previewPath: previewPath);
    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'iLikePDF',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfToImageWorkflow: workflow,
      ),
    );
    await _openPdfToImages(tester);

    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();

    expect(find.text('fixture.pdf'), findsOneWidget);
    expect(find.text('PDF 1 · 2 pages'), findsOneWidget);
    expect(workflow.previewCalls, 1);
    expect(
      find.byKey(const ValueKey(r'selected-pdf-C:\fixtures\fixture.pdf')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('pdf-preview-image')), findsOneWidget);
    expect(find.text('Standard'), findsOneWidget);
    expect(find.text('High'), findsOneWidget);
    expect(find.text('DPI : 150'), findsOneWidget);
    expect(find.text('DPI : 300'), findsOneWidget);
    expect(find.text('Suitable for normal viewing and sharing'), findsNothing);
    expect(
      find.text('Suitable for printing and detailed output'),
      findsNothing,
    );
    expect(find.text('Preview page 1'), findsNothing);
    expect(
      find.text('Preview is for page 1 only. Conversion exports every page.'),
      findsNothing,
    );
  });

  testWidgets(
    'defaults next to sources and preserves a custom folder when adding PDFs',
    (WidgetTester tester) async {
      final workflow = FakePdfToImageWorkflow(previewPath: previewPath);
      await tester.pumpWidget(
        IlikepdfApp(
          applicationName: 'iLikePDF',
          coreVersion: '0.1.0',
          localOnly: true,
          pdfToImageWorkflow: workflow,
        ),
      );
      await _openPdfToImages(tester);

      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();
      expect(find.text('Next to source files'), findsOneWidget);
      expect(find.text(r'C:\fixtures'), findsNothing);

      await tester.ensureVisible(find.text('Custom folder'));
      await tester.tap(find.text('Custom folder'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Choose folder'));
      await tester.tap(find.text('Choose folder'));
      await tester.pumpAndSettle();
      expect(find.text(r'C:\exports'), findsOneWidget);

      workflow.selectedPdf = const SelectedPdf(
        displayName: 'other.pdf',
        sourcePath: r'D:\incoming\other.pdf',
        sourceDirectory: r'D:\incoming',
        pageCount: 1,
        firstPageWidthPoints: 300,
        firstPageHeightPoints: 200,
      );
      await tester.ensureVisible(find.text('Add PDFs'));
      await tester.tap(find.text('Add PDFs'));
      await tester.pumpAndSettle();

      expect(find.text(r'C:\exports'), findsOneWidget);
      expect(find.text('2 selected PDFs'), findsOneWidget);
      expect(workflow.previewCalls, 2);
    },
  );

  testWidgets('exports every page at the selected quality', (
    WidgetTester tester,
  ) async {
    final workflow = FakePdfToImageWorkflow(previewPath: previewPath);
    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'iLikePDF',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfToImageWorkflow: workflow,
      ),
    );
    await _openPdfToImages(tester);

    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();

    await tester.ensureVisible(find.text('High'));
    await tester.tap(find.text('High'));
    await tester.ensureVisible(find.text('Convert to images'));
    await tester.tap(find.text('Convert to images'));
    await tester.pumpAndSettle();

    expect(workflow.exportedQuality, PdfImageQuality.highQuality);
    expect(workflow.exportedFormat, PdfImageFormat.png);
    expect(find.text('Conversion complete'), findsOneWidget);
    expect(find.text('1 PDF completed · 0 PDFs failed'), findsOneWidget);
  });

  testWidgets('shows a structured export failure', (WidgetTester tester) async {
    final workflow = FakePdfToImageWorkflow(
      previewPath: previewPath,
      failure: const PdfExportProblem(
        code: PdfExportProblemCode.outputAlreadyExists,
        message: 'The required output file or folder already exists',
      ),
    );
    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'iLikePDF',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfToImageWorkflow: workflow,
      ),
    );
    await _openPdfToImages(tester);

    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Convert to images'));
    await tester.tap(find.text('Convert to images'));
    await tester.pumpAndSettle();

    expect(find.text('Batch failed'), findsOneWidget);
    expect(
      find.text('The required output file or folder already exists'),
      findsOneWidget,
    );
  });

  testWidgets(
    'a preview failure remains lightweight and does not block export',
    (WidgetTester tester) async {
      final workflow = FakePdfToImageWorkflow(
        previewPath: previewPath,
        previewFails: true,
      );
      await tester.pumpWidget(
        IlikepdfApp(
          applicationName: 'iLikePDF',
          coreVersion: '0.1.0',
          localOnly: true,
          pdfToImageWorkflow: workflow,
        ),
      );
      await _openPdfToImages(tester);

      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();

      expect(find.byKey(const ValueKey('pdf-preview-error')), findsOneWidget);
      expect(find.text('Preview unavailable'), findsOneWidget);
      await tester.ensureVisible(find.text('Convert to images'));
      await tester.tap(find.text('Convert to images'));
      await tester.pumpAndSettle();

      expect(find.text('Conversion complete'), findsOneWidget);
    },
  );
}

Future<void> _openPdfToImages(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('tool-card-pdf-to-images')));
  await tester.pumpAndSettle();
}
