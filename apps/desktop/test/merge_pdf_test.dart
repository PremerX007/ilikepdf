import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/app.dart';
import 'package:ilikepdf/src/app/merge_pdf/merge_pdf_workflow.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_error;

class FakeMergePdfWorkflow implements MergePdfWorkflow {
  FakeMergePdfWorkflow({
    required this.previewPath,
    List<List<SelectedMergePdf>> selections = const [],
    this.preparedSelection = const [],
    this.finalUpdate,
    this.progressGate,
    this.previewFails = false,
  }) : selections = [...selections];

  final String previewPath;
  final List<List<SelectedMergePdf>> selections;
  final List<SelectedMergePdf> preparedSelection;
  final MergePdfUpdate? finalUpdate;
  final Completer<void>? progressGate;
  final bool previewFails;
  final List<String> preparedPaths = [];
  final List<String> previewedPaths = [];
  List<String> mergedPaths = [];
  String? mergedDestination;
  String? mergedOutputName;
  int mergeCalls = 0;

  @override
  Future<String?> chooseDestinationDirectory() async => r'C:\custom-output';

  @override
  Stream<MergePdfUpdate> merge({
    required List<String> sourcePaths,
    required String destinationDirectory,
    required String outputName,
  }) async* {
    mergeCalls += 1;
    mergedPaths = List.unmodifiable(sourcePaths);
    mergedDestination = destinationDirectory;
    mergedOutputName = outputName;
    final pageCount = sourcePaths.fold<int>(0, (total, path) {
      return total + _pdfForPath(path).pageCount;
    });
    yield MergePdfUpdate(
      status: MergePdfUpdateStatus.running,
      stage: MergePdfProgressStage.preparing,
      inputCount: sourcePaths.length,
      totalPageCount: 0,
      outputPath: null,
      warningInputCount: 0,
      hasWarnings: false,
      failedInputIndex: null,
      failedInputPath: null,
      error: null,
    );
    yield MergePdfUpdate(
      status: MergePdfUpdateStatus.running,
      stage: MergePdfProgressStage.merging,
      inputCount: sourcePaths.length,
      totalPageCount: pageCount,
      outputPath: null,
      warningInputCount: 0,
      hasWarnings: false,
      failedInputIndex: null,
      failedInputPath: null,
      error: null,
    );
    if (progressGate case final gate?) await gate.future;
    yield finalUpdate ??
        MergePdfUpdate(
          status: MergePdfUpdateStatus.complete,
          stage: MergePdfProgressStage.completed,
          inputCount: sourcePaths.length,
          totalPageCount: pageCount,
          outputPath: r'C:\custom-output\merged.pdf',
          warningInputCount: 0,
          hasWarnings: false,
          failedInputIndex: null,
          failedInputPath: null,
          error: null,
        );
  }

  @override
  Future<List<SelectedMergePdf>> preparePdfPaths(
    List<String> sourcePaths,
  ) async {
    preparedPaths.addAll(sourcePaths);
    if (sourcePaths.any((path) => !path.toLowerCase().endsWith('.pdf'))) {
      throw const MergePdfSelectionException(
        MergePdfProblem(
          code: rust_error.ApplicationErrorCode.invalidRequest,
          message: 'Only PDF documents are supported.',
        ),
      );
    }
    return preparedSelection;
  }

  @override
  Future<RenderedMergePdfPage> renderFirstPage(String sourcePath) async {
    previewedPaths.add(sourcePath);
    if (previewFails) throw StateError('preview failed');
    return RenderedMergePdfPage(outputPath: previewPath);
  }

  @override
  Future<List<SelectedMergePdf>> selectPdfs() async =>
      selections.isEmpty ? const [] : selections.removeAt(0);

  SelectedMergePdf _pdfForPath(String path) => [
    firstPdf,
    secondPdf,
    thirdPdf,
  ].firstWhere((pdf) => pdf.sourcePath.toLowerCase() == path.toLowerCase());
}

const firstPdf = SelectedMergePdf(
  displayName: 'invoice.pdf',
  sourcePath: r'C:\docs\invoice.pdf',
  sourceDirectory: r'C:\docs',
  pageCount: 2,
);

const secondPdf = SelectedMergePdf(
  displayName: 'cover.pdf',
  sourcePath: r'D:\reports\cover.pdf',
  sourceDirectory: r'D:\reports',
  pageCount: 1,
);

const thirdPdf = SelectedMergePdf(
  displayName: 'appendix.pdf',
  sourcePath: r'E:\incoming\appendix.pdf',
  sourceDirectory: r'E:\incoming',
  pageCount: 3,
);

void main() {
  late Directory previewDirectory;
  late String previewPath;

  setUpAll(() async {
    previewDirectory = await Directory.systemTemp.createTemp(
      'ilikepdf-merge-widget-',
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

  test('normalizes safe PDF filenames and rejects path-like values', () {
    expect(normalizeMergeOutputName('report'), 'report.pdf');
    expect(normalizeMergeOutputName('report.pdf'), 'report.pdf');
    for (final value in [
      '../report.pdf',
      r'..\report.pdf',
      r'C:\report.pdf',
      'foo/bar.pdf',
      'CON.pdf',
    ]) {
      expect(normalizeMergeOutputName(value), isNull);
    }
  });

  testWidgets('empty workspace uses merged.pdf and requires two inputs', (
    WidgetTester tester,
  ) async {
    await _pumpPanel(tester, FakeMergePdfWorkflow(previewPath: previewPath));

    expect(find.byKey(const ValueKey('empty-merge-pdf-drop-zone')), findsOne);
    expect(find.text('Drop PDFs here'), findsOne);
    expect(find.text('Select PDFs'), findsOne);
    expect(find.text('merged.pdf'), findsOne);
    expect(find.text('0 PDFs • 0 pages'), findsOne);
    expect(_primaryButton(tester).onPressed, isNull);
  });

  testWidgets('duplicate inputs are independent cards and count twice', (
    WidgetTester tester,
  ) async {
    final workflow = FakeMergePdfWorkflow(
      previewPath: previewPath,
      selections: const [
        [firstPdf, secondPdf, firstPdf],
      ],
    );
    await _pumpPanel(tester, workflow);

    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();

    expect(find.text('3 selected PDFs'), findsOne);
    expect(find.text('invoice.pdf'), findsNWidgets(2));
    expect(find.byKey(const ValueKey('merge-pdf-card-0')), findsOne);
    expect(find.byKey(const ValueKey('merge-pdf-card-2')), findsOne);
    expect(find.text('3 PDFs • 5 pages'), findsOne);
    expect(find.text(r'C:\docs'), findsOne);
    expect(workflow.previewedPaths, [
      firstPdf.sourcePath,
      secondPdf.sourcePath,
      firstPdf.sourcePath,
    ]);
    expect(_primaryButton(tester).onPressed, isNotNull);

    await tester.tap(find.byKey(const ValueKey('remove-merge-pdf-2')));
    await tester.pumpAndSettle();
    expect(find.text('invoice.pdf'), findsOne);
    expect(find.text('2 PDFs • 3 pages'), findsOne);

    await tester.tap(find.byKey(const ValueKey('remove-merge-pdf-1')));
    await tester.pumpAndSettle();
    expect(find.text('1 PDF • 2 pages'), findsOne);
    expect(_primaryButton(tester).onPressed, isNull);
  });

  testWidgets(
    'custom destination survives add remove reorder and order drives merge',
    (WidgetTester tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 800);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final workflow = FakeMergePdfWorkflow(
        previewPath: previewPath,
        selections: const [
          [firstPdf, secondPdf],
          [thirdPdf],
        ],
      );
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();
      expect(find.text(r'C:\docs'), findsOne);

      final initialGesture = await tester.startGesture(
        tester.getCenter(find.byKey(const ValueKey('reorder-merge-pdf-0'))),
      );
      await tester.pump();
      await initialGesture.moveTo(
        tester.getCenter(find.byKey(const ValueKey('reorder-target-1'))),
      );
      await tester.pump(const Duration(milliseconds: 500));
      await initialGesture.up();
      await tester.pumpAndSettle();
      expect(find.text(r'C:\docs'), findsOne);

      await tester.tap(find.text('Choose folder'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Add PDFs'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('remove-merge-pdf-0')));
      await tester.pumpAndSettle();

      final gesture = await tester.startGesture(
        tester.getCenter(find.byKey(const ValueKey('reorder-merge-pdf-0'))),
      );
      await tester.pump();
      await gesture.moveTo(
        tester.getCenter(find.byKey(const ValueKey('reorder-target-1'))),
      );
      await tester.pump(const Duration(milliseconds: 500));
      await gesture.up();
      await tester.pumpAndSettle();

      expect(find.text(r'C:\custom-output'), findsOne);
      await tester.enterText(
        find.byKey(const ValueKey('merge-output-name')),
        'financial-report',
      );
      await tester.tap(find.byKey(const ValueKey('merge-pdf-action')));
      await tester.pumpAndSettle();

      expect(workflow.mergedPaths, [thirdPdf.sourcePath, firstPdf.sourcePath]);
      expect(workflow.mergedDestination, r'C:\custom-output');
      expect(workflow.mergedOutputName, 'financial-report.pdf');
      expect(find.text('financial-report.pdf'), findsOne);
      expect(find.byKey(const ValueKey('merge-success')), findsOne);
    },
  );

  testWidgets('preview failure stays mergeable and progress is indeterminate', (
    WidgetTester tester,
  ) async {
    final gate = Completer<void>();
    final workflow = FakeMergePdfWorkflow(
      previewPath: previewPath,
      selections: const [
        [firstPdf, secondPdf],
      ],
      progressGate: gate,
      previewFails: true,
    );
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    expect(find.text('Preview unavailable'), findsNWidgets(2));

    await tester.ensureVisible(find.byKey(const ValueKey('merge-pdf-action')));
    await tester.tap(find.byKey(const ValueKey('merge-pdf-action')));
    await tester.pump();
    expect(find.text('Merging 3 pages from 2 PDFs…'), findsOne);
    expect(find.byType(CircularProgressIndicator), findsWidgets);
    expect(_primaryButton(tester).onPressed, isNull);
    await tester.tap(find.text('Merging PDF…'));
    expect(workflow.mergeCalls, 1);

    gate.complete();
    await tester.pumpAndSettle();
    expect(find.text('Merged successfully'), findsOne);
  });

  testWidgets('drop uses shared ingestion and failure identifies the input', (
    WidgetTester tester,
  ) async {
    final workflow = FakeMergePdfWorkflow(
      previewPath: previewPath,
      preparedSelection: const [firstPdf, secondPdf],
      finalUpdate: MergePdfUpdate(
        status: MergePdfUpdateStatus.failed,
        stage: MergePdfProgressStage.preparing,
        inputCount: 2,
        totalPageCount: 2,
        outputPath: null,
        warningInputCount: 0,
        hasWarnings: false,
        failedInputIndex: 1,
        failedInputPath: secondPdf.sourcePath,
        error: MergePdfProblem(
          code: rust_error.ApplicationErrorCode.passwordRequired,
          message: 'This PDF is password protected. Unlock it before merging',
        ),
      ),
    );
    await _pumpPanel(tester, workflow);

    await _dropFiles(tester, [firstPdf.sourcePath, secondPdf.sourcePath]);
    expect(workflow.preparedPaths, [firstPdf.sourcePath, secondPdf.sourcePath]);
    await tester.ensureVisible(find.byKey(const ValueKey('merge-pdf-action')));
    await tester.tap(find.byKey(const ValueKey('merge-pdf-action')));
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('merge-failure')), findsOne);
    expect(find.textContaining('cover.pdf'), findsWidgets);
    expect(find.textContaining('password protected'), findsOne);
  });
}

Future<void> _pumpPanel(
  WidgetTester tester,
  FakeMergePdfWorkflow workflow,
) async {
  await tester.pumpWidget(
    IlikepdfApp(
      applicationName: 'iLikePDF',
      coreVersion: '0.1.0',
      localOnly: true,
      pdfToImageWorkflow: _NoopPdfWorkflow(),
      mergePdfWorkflow: workflow,
    ),
  );
  await tester.tap(find.byKey(const ValueKey('tool-card-merge-pdf')));
  await tester.pumpAndSettle();
}

class _NoopPdfWorkflow implements PdfToImageWorkflow {
  @override
  Future<String?> chooseDestinationDirectory() async => null;

  @override
  Stream<PdfBatchExportUpdate> exportBatch({
    required List<String> sourcePaths,
    required PdfDestinationMode destinationMode,
    required String? customDestinationDirectory,
    required PdfImageQuality quality,
    required PdfImageFormat format,
  }) => const Stream.empty();

  @override
  Future<List<SelectedPdf>> preparePdfPaths(List<String> sourcePaths) async =>
      const [];

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) =>
      throw UnimplementedError();

  @override
  Future<List<SelectedPdf>> selectPdfs() async => const [];
}

FilledButton _primaryButton(WidgetTester tester) => tester.widget<FilledButton>(
  find.descendant(
    of: find.byKey(const ValueKey('merge-pdf-action')),
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
