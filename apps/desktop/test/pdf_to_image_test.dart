import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/app.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';

class FakeBatchPdfWorkflow implements PdfToImageWorkflow {
  FakeBatchPdfWorkflow({
    required this.previewPath,
    List<List<SelectedPdf>> selections = const [],
    this.preparedSelection = const [],
    this.finalUpdate,
    this.progressGate,
  }) : selections = [...selections];

  final String previewPath;
  final List<List<SelectedPdf>> selections;
  final List<SelectedPdf> preparedSelection;
  final PdfBatchExportUpdate? finalUpdate;
  final Completer<void>? progressGate;
  final List<String> preparedDroppedPaths = [];
  final List<String> previewedPaths = [];
  List<String> exportedSourcePaths = const [];
  PdfDestinationMode? exportedDestinationMode;
  String? exportedCustomDestination;
  PdfImageQuality? exportedQuality;
  PdfImageFormat? exportedFormat;
  int exportCallCount = 0;

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
    exportCallCount += 1;
    exportedSourcePaths = List.unmodifiable(sourcePaths);
    exportedDestinationMode = destinationMode;
    exportedCustomDestination = customDestinationDirectory;
    exportedQuality = quality;
    exportedFormat = format;
    final totalPages = sourcePaths.fold<int>(
      0,
      (sum, path) => sum + _pdfForPath(path).pageCount,
    );
    yield PdfBatchExportUpdate(
      status: PdfBatchExportStatus.running,
      totalDocumentCount: sourcePaths.length,
      completedDocumentCount: sourcePaths.length > 1 ? 1 : 0,
      succeededDocumentCount: 0,
      failedDocumentCount: 0,
      currentDocumentIndex: sourcePaths.length > 1 ? 2 : 1,
      currentDocumentFilename: _pdfForPath(
        sourcePaths[sourcePaths.length > 1 ? 1 : 0],
      ).displayName,
      totalPageCount: totalPages,
      completedPageCount: totalPages > 0 ? 1 : 0,
      currentPage: 1,
      documents: const [],
      error: null,
    );
    if (progressGate case final gate?) {
      await gate.future;
    }
    yield finalUpdate ?? _successfulUpdate(sourcePaths);
  }

  @override
  Future<List<SelectedPdf>> preparePdfPaths(List<String> sourcePaths) async {
    preparedDroppedPaths.addAll(sourcePaths);
    if (sourcePaths.any((path) => !path.toLowerCase().endsWith('.pdf'))) {
      throw const PdfSelectionException(
        PdfExportProblem(
          code: PdfExportProblemCode.invalidRequest,
          message: 'Only PDF documents are supported.',
        ),
      );
    }
    return preparedSelection;
  }

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) async {
    previewedPaths.add(sourcePath);
    return RenderedPdfPage(
      outputPath: previewPath,
      widthPixels: 1,
      heightPixels: 1,
    );
  }

  @override
  Future<List<SelectedPdf>> selectPdfs() async =>
      selections.isEmpty ? const [] : selections.removeAt(0);

  SelectedPdf _pdfForPath(String path) =>
      [firstPdf, secondPdf, thirdPdf, invalidPdf].firstWhere(
        (pdf) => pdf.sourcePath.toLowerCase() == path.toLowerCase(),
        orElse: () => SelectedPdf(
          displayName: File(path).uri.pathSegments.last,
          sourcePath: path,
          sourceDirectory: File(path).parent.path,
          pageCount: 1,
          firstPageWidthPoints: null,
          firstPageHeightPoints: null,
        ),
      );

  PdfBatchExportUpdate _successfulUpdate(List<String> paths) {
    final documents = paths
        .map((path) {
          final pdf = _pdfForPath(path);
          return PdfBatchDocumentOutcome(
            sourcePath: path,
            displayName: pdf.displayName,
            totalPageCount: pdf.pageCount,
            completedPageCount: pdf.pageCount,
            outputFiles: [r'C:\exports\output.png'],
            error: null,
          );
        })
        .toList(growable: false);
    final pages = documents.fold<int>(
      0,
      (sum, document) => sum + document.completedPageCount,
    );
    return PdfBatchExportUpdate(
      status: PdfBatchExportStatus.complete,
      totalDocumentCount: paths.length,
      completedDocumentCount: paths.length,
      succeededDocumentCount: paths.length,
      failedDocumentCount: 0,
      currentDocumentIndex: null,
      currentDocumentFilename: null,
      totalPageCount: pages,
      completedPageCount: pages,
      currentPage: null,
      documents: documents,
      error: null,
    );
  }
}

const firstPdf = SelectedPdf(
  displayName: 'invoice.pdf',
  sourcePath: r'C:\docs\invoice.pdf',
  sourceDirectory: r'C:\docs',
  pageCount: 2,
  firstPageWidthPoints: 300,
  firstPageHeightPoints: 200,
);

const secondPdf = SelectedPdf(
  displayName: 'cover.pdf',
  sourcePath: r'D:\reports\cover.pdf',
  sourceDirectory: r'D:\reports',
  pageCount: 1,
  firstPageWidthPoints: 300,
  firstPageHeightPoints: 200,
);

const thirdPdf = SelectedPdf(
  displayName: 'summary.pdf',
  sourcePath: r'E:\incoming\summary.pdf',
  sourceDirectory: r'E:\incoming',
  pageCount: 3,
  firstPageWidthPoints: 300,
  firstPageHeightPoints: 200,
);

const invalidPdf = SelectedPdf(
  displayName: 'broken.pdf',
  sourcePath: r'C:\docs\broken.pdf',
  sourceDirectory: r'C:\docs',
  pageCount: 0,
  firstPageWidthPoints: null,
  firstPageHeightPoints: null,
  inspectionProblem: PdfExportProblem(
    code: PdfExportProblemCode.invalidPdf,
    message: 'The selected file is not a valid readable PDF',
  ),
);

void main() {
  late Directory previewDirectory;
  late String previewPath;

  setUpAll(() async {
    previewDirectory = await Directory.systemTemp.createTemp(
      'ilikepdf-pdf-batch-widget-',
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

  testWidgets('empty state exposes batch selection and concise defaults', (
    WidgetTester tester,
  ) async {
    final workflow = FakeBatchPdfWorkflow(previewPath: previewPath);
    await _pumpPanel(tester, workflow);

    expect(find.byKey(const ValueKey('empty-pdf-drop-zone')), findsOneWidget);
    expect(find.text('Drop PDFs here'), findsOneWidget);
    expect(find.text('Select PDFs'), findsOneWidget);
    expect(find.text('Standard'), findsOneWidget);
    expect(find.text('DPI : 150'), findsOneWidget);
    expect(find.text('High'), findsOneWidget);
    expect(find.text('DPI : 300'), findsOneWidget);
    expect(find.text('Output format'), findsOneWidget);
    expect(find.text('PNG'), findsOneWidget);
    expect(find.text('JPG'), findsOneWidget);
    expect(
      tester
          .widget<SegmentedButton<PdfImageFormat>>(
            find.byKey(const ValueKey('pdf-output-format')),
          )
          .selected,
      {PdfImageFormat.png},
    );
    expect(find.text('Next to source files'), findsOneWidget);
    expect(find.text('Custom folder'), findsOneWidget);
    expect(find.byKey(const ValueKey('destination-path')), findsNothing);
    expect(_primaryButton(tester).onPressed, isNull);
  });

  testWidgets(
    'selecting multiple PDFs creates preview cards and supports removal',
    (WidgetTester tester) async {
      final workflow = FakeBatchPdfWorkflow(
        previewPath: previewPath,
        selections: const [
          [firstPdf, secondPdf],
        ],
      );
      await _pumpPanel(tester, workflow);

      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();

      expect(find.text('2 selected PDFs'), findsOneWidget);
      expect(find.text('invoice.pdf'), findsOneWidget);
      expect(find.text('cover.pdf'), findsOneWidget);
      expect(find.text('PDF 1 · 2 pages'), findsOneWidget);
      expect(find.text('PDF 2 · 1 page'), findsOneWidget);
      expect(find.byKey(const ValueKey('pdf-preview-image')), findsNWidgets(2));
      expect(workflow.previewedPaths, [
        firstPdf.sourcePath,
        secondPdf.sourcePath,
      ]);
      expect(_primaryButton(tester).onPressed, isNotNull);

      await tester.tap(find.byKey(const ValueKey('remove-pdf-1')));
      await tester.pumpAndSettle();
      expect(find.text('1 selected PDF'), findsOneWidget);
      expect(find.text('cover.pdf'), findsNothing);
    },
  );

  testWidgets(
    'custom destination persists through add remove and reorder and order drives export',
    (WidgetTester tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 800);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final workflow = FakeBatchPdfWorkflow(
        previewPath: previewPath,
        selections: const [
          [firstPdf, secondPdf],
          [thirdPdf],
        ],
      );
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();

      await tester.ensureVisible(find.text('Custom folder'));
      await tester.tap(find.text('Custom folder'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Choose folder'));
      await tester.pumpAndSettle();
      expect(find.text(r'C:\exports'), findsOneWidget);

      await tester.tap(find.text('Add PDFs'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('remove-pdf-1')));
      await tester.pumpAndSettle();

      final handle = find.byKey(const ValueKey('reorder-pdf-0'));
      final gesture = await tester.startGesture(tester.getCenter(handle));
      await tester.pump();
      await gesture.moveTo(
        tester.getCenter(find.byKey(const ValueKey('reorder-target-1'))),
      );
      await tester.pump(const Duration(milliseconds: 500));
      await gesture.up();
      await tester.pumpAndSettle();

      expect(find.text(r'C:\exports'), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('quality-High')));
      await tester.tap(find.text('JPG'));
      await tester.tap(find.text('Convert to images'));
      await tester.pumpAndSettle();

      expect(workflow.exportedSourcePaths, [
        thirdPdf.sourcePath,
        firstPdf.sourcePath,
      ]);
      expect(workflow.exportedDestinationMode, PdfDestinationMode.customFolder);
      expect(workflow.exportedCustomDestination, r'C:\exports');
      expect(workflow.exportedQuality, PdfImageQuality.highQuality);
      expect(workflow.exportedFormat, PdfImageFormat.jpg);
      expect(find.text('Conversion complete'), findsOneWidget);
    },
  );

  testWidgets(
    'native drop uses normal ingestion and rejects unsupported files',
    (WidgetTester tester) async {
      final workflow = FakeBatchPdfWorkflow(
        previewPath: previewPath,
        preparedSelection: const [firstPdf],
      );
      await _pumpPanel(tester, workflow);

      await _dropFiles(tester, [firstPdf.sourcePath]);
      expect(workflow.preparedDroppedPaths, [firstPdf.sourcePath]);
      expect(find.text('invoice.pdf'), findsOneWidget);

      await _dropFiles(tester, [r'C:\docs\notes.txt']);
      expect(find.text('Only PDF documents are supported.'), findsOneWidget);
      expect(find.text('invoice.pdf'), findsOneWidget);
    },
  );

  testWidgets('overall progress becomes a mixed per-document result summary', (
    WidgetTester tester,
  ) async {
    final gate = Completer<void>();
    final partial = PdfBatchExportUpdate(
      status: PdfBatchExportStatus.completeWithErrors,
      totalDocumentCount: 3,
      completedDocumentCount: 3,
      succeededDocumentCount: 2,
      failedDocumentCount: 1,
      currentDocumentIndex: null,
      currentDocumentFilename: null,
      totalPageCount: 5,
      completedPageCount: 5,
      currentPage: null,
      documents: [
        PdfBatchDocumentOutcome(
          sourcePath: firstPdf.sourcePath,
          displayName: 'invoice.pdf',
          totalPageCount: 2,
          completedPageCount: 2,
          outputFiles: [r'C:\exports\invoice\invoice-page-0001.png'],
          error: null,
        ),
        PdfBatchDocumentOutcome(
          sourcePath: invalidPdf.sourcePath,
          displayName: 'broken.pdf',
          totalPageCount: 0,
          completedPageCount: 0,
          outputFiles: [],
          error: PdfExportProblem(
            code: PdfExportProblemCode.invalidPdf,
            message: 'The selected file is not a valid readable PDF',
          ),
        ),
        PdfBatchDocumentOutcome(
          sourcePath: thirdPdf.sourcePath,
          displayName: 'summary.pdf',
          totalPageCount: 3,
          completedPageCount: 3,
          outputFiles: [r'C:\exports\summary\summary-page-0001.png'],
          error: null,
        ),
      ],
      error: null,
    );
    final workflow = FakeBatchPdfWorkflow(
      previewPath: previewPath,
      selections: const [
        [firstPdf, invalidPdf, thirdPdf],
      ],
      finalUpdate: partial,
      progressGate: gate,
    );
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('pdf-inspection-error')), findsOneWidget);

    await tester.ensureVisible(
      find.byKey(const ValueKey('convert-to-images-action')),
    );
    await tester.tap(find.text('Convert to images'));
    await tester.pump();

    expect(find.byKey(const ValueKey('feedback-running')), findsOneWidget);
    expect(find.text('Converting PDFs'), findsOneWidget);
    expect(find.text('PDF 2 of 3'), findsOneWidget);
    expect(find.text('Page 2 of 5'), findsOneWidget);
    expect(_primaryButton(tester).onPressed, isNull);
    await tester.tap(find.text('Converting PDFs…'));
    expect(workflow.exportCallCount, 1);

    gate.complete();
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('batch-summary-partial')), findsOneWidget);
    expect(find.text('Completed with errors'), findsOneWidget);
    expect(find.text('2 PDFs completed · 1 PDF failed'), findsOneWidget);
    expect(find.text('invoice.pdf'), findsNWidgets(2));
    expect(find.text('broken.pdf'), findsNWidgets(2));
    expect(find.text('summary.pdf'), findsNWidgets(2));
    expect(
      find.text('The selected file is not a valid readable PDF'),
      findsWidgets,
    );
  });
}

Future<void> _pumpPanel(
  WidgetTester tester,
  FakeBatchPdfWorkflow workflow,
) async {
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
}

FilledButton _primaryButton(WidgetTester tester) => tester.widget<FilledButton>(
  find.descendant(
    of: find.byKey(const ValueKey('convert-to-images-action')),
    matching: find.byType(FilledButton),
  ),
);

Future<void> _dropFiles(WidgetTester tester, List<String> paths) async {
  final target = tester.widget<DropTarget>(find.byType(DropTarget));
  target.onDragDone?.call(
    DropDoneDetails(
      files: paths.map(DropItemFile.new).toList(growable: false),
      localPosition: Offset.zero,
      globalPosition: Offset.zero,
    ),
  );
  await tester.pumpAndSettle();
}
