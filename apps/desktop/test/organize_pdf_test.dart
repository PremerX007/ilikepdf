import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/organize_pdf/organize_pdf_panel.dart';
import 'package:ilikepdf/src/app/organize_pdf/organize_pdf_workflow.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

const firstPdf = SelectedOrganizePdf(
  displayName: 'A.pdf',
  sourcePath: r'C:\docs\A.pdf',
  sourceDirectory: r'C:\docs',
  pageCount: 2,
  hasWarnings: false,
);
const secondPdf = SelectedOrganizePdf(
  displayName: 'B.pdf',
  sourcePath: r'D:\incoming\B.pdf',
  sourceDirectory: r'D:\incoming',
  pageCount: 1,
  hasWarnings: false,
);
const thirdPdf = SelectedOrganizePdf(
  displayName: 'C.pdf',
  sourcePath: r'E:\later\C.pdf',
  sourceDirectory: r'E:\later',
  pageCount: 1,
  hasWarnings: false,
);

class FakeOrganizePdfWorkflow implements OrganizePdfWorkflow {
  FakeOrganizePdfWorkflow({required this.previewPath});

  final String previewPath;
  final List<List<SelectedOrganizePdf>> selections = [];
  List<SelectedOrganizePdf> prepared = const [];
  OrganizePdfProblem? selectionProblem;
  Completer<void>? progressGate;
  bool failOrganize = false;
  int selectionCalls = 0;
  int organizeCalls = 0;
  int renderCalls = 0;
  List<String> lastExistingPaths = const [];
  List<String> lastCandidatePaths = const [];
  List<OrganizePdfSourceInput> submittedSources = const [];
  List<OrganizePdfPageInput> submittedPages = const [];
  String? submittedDestination;
  String? submittedOutputName;

  @override
  Future<String?> chooseDestinationDirectory() async => r'F:\exports';

  @override
  Future<List<SelectedOrganizePdf>> preparePdfPaths({
    required List<String> existingSourcePaths,
    required List<String> candidateSourcePaths,
  }) async {
    lastExistingPaths = List.unmodifiable(existingSourcePaths);
    lastCandidatePaths = List.unmodifiable(candidateSourcePaths);
    if (selectionProblem case final problem?) {
      throw OrganizePdfSelectionException(problem);
    }
    return prepared;
  }

  @override
  Future<RenderedOrganizePdfPage> renderPage({
    required String sourcePath,
    required int pageIndex,
  }) async {
    renderCalls++;
    final source = File(previewPath);
    final rendered = File(
      '${source.parent.path}\\rendered-thumbnail-$renderCalls.png',
    );
    source.copySync(rendered.path);
    return RenderedOrganizePdfPage(
      outputPath: rendered.path,
      widthPixels: 1,
      heightPixels: 1,
    );
  }

  @override
  Future<List<SelectedOrganizePdf>> selectPdfs({
    required List<String> existingSourcePaths,
  }) async {
    lastExistingPaths = List.unmodifiable(existingSourcePaths);
    if (selectionProblem case final problem?) {
      throw OrganizePdfSelectionException(problem);
    }
    final index = selectionCalls++;
    return index < selections.length ? selections[index] : const [];
  }

  @override
  Stream<OrganizePdfUpdate> organize({
    required List<OrganizePdfSourceInput> sources,
    required List<OrganizePdfPageInput> pageItems,
    required String destinationDirectory,
    required String outputName,
  }) async* {
    organizeCalls++;
    submittedSources = List.unmodifiable(sources);
    submittedPages = List.unmodifiable(pageItems);
    submittedDestination = destinationDirectory;
    submittedOutputName = outputName;
    yield OrganizePdfUpdate(
      status: OrganizePdfUpdateStatus.running,
      stage: OrganizePdfProgressStage.organizing,
      sourceCount: sources.length,
      pageCount: pageItems.length,
      outputPath: null,
      warningSourceCount: 0,
      hasWarnings: false,
      failedSourceId: null,
      failedSourcePath: null,
      failedPageItemId: null,
      error: null,
    );
    await progressGate?.future;
    if (failOrganize) {
      yield OrganizePdfUpdate(
        status: OrganizePdfUpdateStatus.failed,
        stage: OrganizePdfProgressStage.validating,
        sourceCount: sources.length,
        pageCount: pageItems.length,
        outputPath: null,
        warningSourceCount: 0,
        hasWarnings: false,
        failedSourceId: sources.first.sourceId,
        failedSourcePath: sources.first.sourcePath,
        failedPageItemId: null,
        error: const OrganizePdfProblem(
          code: ApplicationErrorCode.invalidPdf,
          message: 'A source PDF changed after it was added.',
        ),
      );
    } else {
      yield OrganizePdfUpdate(
        status: OrganizePdfUpdateStatus.complete,
        stage: OrganizePdfProgressStage.completed,
        sourceCount: sources.length,
        pageCount: pageItems.length,
        outputPath: r'F:\exports\organized.pdf',
        warningSourceCount: 0,
        hasWarnings: false,
        failedSourceId: null,
        failedSourcePath: null,
        failedPageItemId: null,
        error: null,
      );
    }
  }
}

void main() {
  late Directory previewDirectory;
  late String previewPath;

  setUpAll(() async {
    previewDirectory = await Directory.systemTemp.createTemp(
      'ilikepdf-organize-widget-preview-',
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

  test('output name normalization rejects path-like and reserved names', () {
    expect(normalizeOrganizeOutputName('report'), 'report.pdf');
    expect(normalizeOrganizeOutputName('report.pdf'), 'report.pdf');
    expect(normalizeOrganizeOutputName('../report.pdf'), isNull);
    expect(normalizeOrganizeOutputName(r'C:\report.pdf'), isNull);
    expect(normalizeOrganizeOutputName('NUL.pdf'), isNull);
  });

  testWidgets('empty state loads multiple PDFs and appends later files', (
    tester,
  ) async {
    final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
      ..selections.addAll([
        [firstPdf, secondPdf],
        [thirdPdf],
      ]);
    await _pumpPanel(tester, workflow);

    expect(
      find.byKey(const ValueKey('empty-organize-pdf-drop-zone')),
      findsOne,
    );
    expect(_primaryAction(tester).onPressed, isNull);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();

    expect(find.text('3 pages from 2 PDFs'), findsOne);
    expect(find.text('A.pdf'), findsWidgets);
    expect(find.text('B.pdf'), findsWidgets);
    expect(find.text(r'C:\docs'), findsOne);
    expect(find.byKey(const ValueKey('organize-page-card-0')), findsOne);
    expect(find.byKey(const ValueKey('organize-page-card-2')), findsOne);
    expect(_primaryAction(tester).onPressed, isNotNull);

    await tester.tap(find.byKey(const ValueKey('add-organize-pdfs')));
    await tester.pumpAndSettle();
    expect(workflow.lastExistingPaths, [
      firstPdf.sourcePath,
      secondPdf.sourcePath,
    ]);
    expect(find.text('4 pages from 3 PDFs'), findsOne);
    expect(find.byKey(const ValueKey('organize-page-card-3')), findsOne);
    expect(find.text(r'C:\docs'), findsOne);
  });

  testWidgets(
    'drop rejection keeps session and source removal removes its pages',
    (tester) async {
      final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
        ..selections.add([firstPdf, secondPdf]);
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();

      workflow.selectionProblem = const OrganizePdfProblem(
        code: ApplicationErrorCode.duplicateSource,
        message: 'This PDF has already been added to the organize session',
      );
      final dropZone = tester.widget<FileDropZone>(find.byType(FileDropZone));
      await dropZone.onDroppedPaths([firstPdf.sourcePath]);
      await tester.pumpAndSettle();

      expect(workflow.lastCandidatePaths, [firstPdf.sourcePath]);
      expect(find.textContaining('already been added'), findsOne);
      expect(find.text('3 pages from 2 PDFs'), findsOne);

      await tester.tap(find.byKey(const ValueKey('remove-organize-source-0')));
      await tester.pumpAndSettle();
      expect(find.text('1 page from 1 PDF'), findsOne);
      expect(find.byKey(const ValueKey('organize-page-card-0')), findsNothing);
      expect(find.byKey(const ValueKey('organize-page-card-1')), findsNothing);
      expect(find.byKey(const ValueKey('organize-page-card-2')), findsOne);
    },
  );

  testWidgets('reorder delete rotate and Reset all update stable page items', (
    tester,
  ) async {
    final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
      ..selections.add([firstPdf, secondPdf]);
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('rotate-right-0')));
    await tester.pump();
    expect(
      tester
          .widget<RotatedBox>(
            find.byKey(const ValueKey('organize-page-rotation-0')),
          )
          .quarterTurns,
      1,
    );

    final gesture = await tester.startGesture(
      tester.getCenter(find.byKey(const ValueKey('reorder-organize-page-2'))),
    );
    await tester.pump();
    await gesture.moveTo(
      tester.getCenter(find.byKey(const ValueKey('lazy-reorder-target-0'))),
    );
    await tester.pump(const Duration(milliseconds: 500));
    await gesture.up();
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('delete-organize-page-1')));
    await tester.pumpAndSettle();
    expect(find.text('2 pages from 2 PDFs'), findsOne);
    expect(find.text('A.pdf'), findsWidgets);
    expect(find.text('B.pdf'), findsWidgets);

    await tester.ensureVisible(
      find.byKey(const ValueKey('reset-organize-all')),
    );
    await tester.tap(find.byKey(const ValueKey('reset-organize-all')));
    await tester.pumpAndSettle();
    expect(find.text('3 pages from 2 PDFs'), findsOne);
    expect(
      tester
          .widget<RotatedBox>(
            find.byKey(const ValueKey('organize-page-rotation-0')),
          )
          .quarterTurns,
      0,
    );
  });

  testWidgets(
    'large sessions render only a bounded near-visible thumbnail set',
    (tester) async {
      final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
        ..selections.add([
          const SelectedOrganizePdf(
            displayName: 'large.pdf',
            sourcePath: r'C:\docs\large.pdf',
            sourceDirectory: r'C:\docs',
            pageCount: 100,
            hasWarnings: false,
          ),
        ]);
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();

      expect(find.text('100 pages from 1 PDF'), findsOne);
      expect(workflow.renderCalls, lessThanOrEqualTo(32));
      expect(workflow.renderCalls, lessThan(100));
    },
  );

  testWidgets('custom destination and name persist into one organized output', (
    tester,
  ) async {
    final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
      ..selections.addAll([
        [firstPdf],
        [secondPdf],
      ]);
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Choose folder'));
    await tester.tap(find.text('Choose folder'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('add-organize-pdfs')));
    await tester.pumpAndSettle();
    await tester.ensureVisible(
      find.byKey(const ValueKey('organize-output-name')),
    );
    await tester.enterText(
      find.byKey(const ValueKey('organize-output-name')),
      'assembled',
    );
    await tester.ensureVisible(
      find.byKey(const ValueKey('organize-pdf-action')),
    );
    await tester.tap(find.byKey(const ValueKey('organize-pdf-action')));
    await tester.pumpAndSettle();

    expect(workflow.organizeCalls, 1);
    expect(workflow.submittedDestination, r'F:\exports');
    expect(workflow.submittedOutputName, 'assembled.pdf');
    expect(workflow.submittedSources.length, 2);
    expect(workflow.submittedPages.length, 3);
    expect(find.byKey(const ValueKey('organize-success')), findsOne);
    expect(find.text('Organized successfully'), findsOne);
    expect(find.textContaining(r'F:\exports\organized.pdf'), findsOne);
  });

  testWidgets('running state blocks duplicate submission and failure renders', (
    tester,
  ) async {
    final gate = Completer<void>();
    final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
      ..selections.add([firstPdf])
      ..progressGate = gate
      ..failOrganize = true;
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(
      find.byKey(const ValueKey('organize-pdf-action')),
    );
    await tester.tap(find.byKey(const ValueKey('organize-pdf-action')));
    await tester.pump();

    expect(find.text('Organizing PDF…'), findsWidgets);
    expect(_primaryAction(tester).onPressed, isNull);
    await tester.tap(find.byKey(const ValueKey('organize-pdf-action')));
    expect(workflow.organizeCalls, 1);

    gate.complete();
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('organize-failure')), findsOne);
    expect(find.textContaining('source PDF changed'), findsOne);
  });
}

Future<void> _pumpPanel(
  WidgetTester tester,
  FakeOrganizePdfWorkflow workflow,
) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(1280, 820);
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(body: OrganizePdfPanel(workflow: workflow)),
    ),
  );
}

PrimaryToolAction _primaryAction(WidgetTester tester) =>
    tester.widget<PrimaryToolAction>(
      find.byKey(const ValueKey('organize-pdf-action')),
    );
