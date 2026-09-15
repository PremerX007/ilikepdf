import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';
import 'package:ilikepdf/src/app/split_pdf/split_pdf_panel.dart';
import 'package:ilikepdf/src/app/split_pdf/split_pdf_workflow.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

const firstPdf = SelectedSplitPdf(
  displayName: 'report.pdf',
  sourcePath: r'C:\docs\report.pdf',
  sourceDirectory: r'C:\docs',
  pageCount: 20,
  sizeBytes: 25000000,
  hasWarnings: false,
);

const secondPdf = SelectedSplitPdf(
  displayName: 'replacement.pdf',
  sourcePath: r'D:\incoming\replacement.pdf',
  sourceDirectory: r'D:\incoming',
  pageCount: 8,
  sizeBytes: 12000000,
  hasWarnings: false,
);

class FakeSplitPdfWorkflow implements SplitPdfWorkflow {
  FakeSplitPdfWorkflow({required this.previewPath});

  final String previewPath;
  SelectedSplitPdf? selected = firstPdf;
  bool rejectPrepared = false;
  String chosenDestination = r'E:\exports';
  SplitPdfMode? submittedMode;
  int? submittedEveryN;
  String? submittedSplitAfter;
  int? submittedMaximumMb;

  @override
  Future<String?> chooseDestinationDirectory() async => chosenDestination;

  @override
  Future<SelectedSplitPdf> preparePdfPaths(List<String> sourcePaths) async {
    if (sourcePaths.length != 1) {
      throw const SplitPdfSelectionException(
        SplitPdfProblem(
          code: ApplicationErrorCode.invalidRequest,
          message: 'Split PDF supports one PDF at a time.',
        ),
      );
    }
    if (rejectPrepared) {
      throw const SplitPdfSelectionException(
        SplitPdfProblem(
          code: ApplicationErrorCode.invalidPdf,
          message: 'The selected file is not a structurally valid PDF.',
        ),
      );
    }
    return selected!;
  }

  @override
  Future<RenderedSplitPdfPage> renderFirstPage(String sourcePath) async =>
      RenderedSplitPdfPage(outputPath: previewPath);

  @override
  Future<SelectedSplitPdf?> selectPdf() async => selected;

  @override
  Stream<SplitPdfUpdate> split({
    required String sourcePath,
    required String destinationDirectory,
    required SplitPdfMode mode,
    required int? everyNPages,
    required String? splitAfterPages,
    required int? maximumSizeMb,
  }) async* {
    submittedMode = mode;
    submittedEveryN = everyNPages;
    submittedSplitAfter = splitAfterPages;
    submittedMaximumMb = maximumSizeMb;
    yield const SplitPdfUpdate(
      status: SplitPdfUpdateStatus.running,
      stage: SplitPdfProgressStage.creating,
      sourcePageCount: 20,
      currentPart: 1,
      totalParts: 2,
      outputDirectory: null,
      parts: [],
      hasWarnings: false,
      failedPageNumber: null,
      actualSizeBytes: null,
      limitSizeBytes: null,
      error: null,
    );
    yield const SplitPdfUpdate(
      status: SplitPdfUpdateStatus.complete,
      stage: SplitPdfProgressStage.completed,
      sourcePageCount: 20,
      currentPart: 2,
      totalParts: 2,
      outputDirectory: r'E:\exports\report-split',
      parts: [
        SplitPdfPart(
          outputPath: r'E:\exports\report-split\report-part-0001.pdf',
          range: SplitPdfRange(firstPage: 1, lastPage: 10),
          sizeBytes: 8700000,
        ),
        SplitPdfPart(
          outputPath: r'E:\exports\report-split\report-part-0002.pdf',
          range: SplitPdfRange(firstPage: 11, lastPage: 20),
          sizeBytes: 9800000,
        ),
      ],
      hasWarnings: false,
      failedPageNumber: null,
      actualSizeBytes: null,
      limitSizeBytes: null,
      error: null,
    );
  }
}

void main() {
  late Directory previewDirectory;
  late String previewPath;

  setUpAll(() async {
    previewDirectory = await Directory.systemTemp.createTemp(
      'ilikepdf-split-widget-preview-',
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

  test(
    'local ingestion rejects zero and multiple sources before inspection',
    () async {
      const workflow = LocalSplitPdfWorkflow();
      for (final paths in [
        <String>[],
        ['one.pdf', 'two.pdf'],
      ]) {
        await expectLater(
          workflow.preparePdfPaths(paths),
          throwsA(
            isA<SplitPdfSelectionException>().having(
              (error) => error.problem.message,
              'message',
              'Split PDF supports one PDF at a time.',
            ),
          ),
        );
      }
    },
  );

  testWidgets('starts singular with every page selected and four modes', (
    tester,
  ) async {
    await _pumpPanel(tester, FakeSplitPdfWorkflow(previewPath: previewPath));

    expect(find.text('Drop a PDF here'), findsOneWidget);
    expect(find.text('Select PDF'), findsOneWidget);
    expect(find.text('Every page'), findsOneWidget);
    expect(find.text('Every N pages'), findsOneWidget);
    expect(find.text('Split after pages'), findsOneWidget);
    expect(find.text('By maximum file size'), findsOneWidget);
    expect(
      tester
          .widget<ChoiceChip>(
            find.byKey(const ValueKey('split-mode-everyPage')),
          )
          .selected,
      isTrue,
    );
  });

  testWidgets('selection shows one source preview and default destination', (
    tester,
  ) async {
    await _pumpPanel(tester, FakeSplitPdfWorkflow(previewPath: previewPath));
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('split-pdf-source-card')), findsOneWidget);
    expect(find.text('report.pdf'), findsOneWidget);
    expect(find.text('20 pages'), findsWidgets);
    expect(
      find.byKey(const ValueKey('split-pdf-preview-image')),
      findsOneWidget,
    );
    expect(find.text(r'C:\docs'), findsOneWidget);
    expect(find.textContaining('20 PDF files'), findsOneWidget);
    expect(
      tester
          .widget<PrimaryToolAction>(
            find.byKey(const ValueKey('split-pdf-action')),
          )
          .onPressed,
      isNotNull,
    );
  });

  testWidgets(
    'multiple drops and invalid replacement preserve current source',
    (tester) async {
      final workflow = FakeSplitPdfWorkflow(previewPath: previewPath);
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDF'));
      await tester.pumpAndSettle();

      final dropZone = tester.widget<FileDropZone>(find.byType(FileDropZone));
      await dropZone.onDroppedPaths(['first.pdf', 'second.pdf']);
      await tester.pumpAndSettle();
      expect(
        find.text('Split PDF supports one PDF at a time.'),
        findsOneWidget,
      );
      expect(find.text('report.pdf'), findsOneWidget);

      workflow.rejectPrepared = true;
      await tester
          .widget<FileDropZone>(find.byType(FileDropZone))
          .onDroppedPaths(['bad.pdf']);
      await tester.pumpAndSettle();
      expect(find.text('report.pdf'), findsOneWidget);
      expect(
        find.textContaining('not a structurally valid PDF'),
        findsOneWidget,
      );
    },
  );

  testWidgets('valid replacement updates default destination', (tester) async {
    final workflow = FakeSplitPdfWorkflow(previewPath: previewPath);
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
    workflow.selected = secondPdf;

    await tester.tap(find.byKey(const ValueKey('replace-split-pdf')));
    await tester.pumpAndSettle();

    expect(find.text('replacement.pdf'), findsOneWidget);
    expect(find.text(r'D:\incoming'), findsOneWidget);
  });

  testWidgets(
    'custom destination persists across replacement and mode change',
    (tester) async {
      final workflow = FakeSplitPdfWorkflow(previewPath: previewPath);
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDF'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Choose folder'));
      await tester.tap(find.text('Choose folder'));
      await tester.pumpAndSettle();
      workflow.selected = secondPdf;
      await tester.tap(find.byKey(const ValueKey('replace-split-pdf')));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Every N pages'));
      await tester.tap(find.text('Every N pages'));
      await tester.pumpAndSettle();

      expect(find.text(r'E:\exports'), findsOneWidget);
      expect(find.textContaining('Custom destination'), findsOneWidget);
    },
  );

  testWidgets('one-page source explains why split is disabled', (tester) async {
    final workflow = FakeSplitPdfWorkflow(previewPath: previewPath)
      ..selected = const SelectedSplitPdf(
        displayName: 'one.pdf',
        sourcePath: r'C:\docs\one.pdf',
        sourceDirectory: r'C:\docs',
        pageCount: 1,
        sizeBytes: 1000,
        hasWarnings: false,
      );
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();

    expect(find.textContaining('only one page'), findsOneWidget);
    expect(
      tester
          .widget<PrimaryToolAction>(
            find.byKey(const ValueKey('split-pdf-action')),
          )
          .onPressed,
      isNull,
    );
  });

  testWidgets('validates every N and split-after without sorting', (
    tester,
  ) async {
    await _pumpPanel(tester, FakeSplitPdfWorkflow(previewPath: previewPath));
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Every N pages'));
    await tester.tap(find.text('Every N pages'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('split-every-n-input')),
      '20',
    );
    await tester.pump();
    expect(find.text('Enter a value smaller than 20.'), findsOneWidget);
    await tester.enterText(
      find.byKey(const ValueKey('split-every-n-input')),
      'abc',
    );
    await tester.pump();
    expect(find.textContaining('positive whole number'), findsOneWidget);

    await tester.tap(find.text('Split after pages'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('split-after-input')),
      '7,3',
    );
    await tester.pump();
    expect(find.textContaining('ascending'), findsOneWidget);
    await tester.enterText(
      find.byKey(const ValueKey('split-after-input')),
      '3, 7, 15',
    );
    await tester.pump();
    expect(find.textContaining('4 PDF files'), findsOneWidget);
    expect(find.textContaining('Pages 16–20'), findsOneWidget);
  });

  testWidgets('size mode defaults to decimal 10 MB and defers part discovery', (
    tester,
  ) async {
    await _pumpPanel(tester, FakeSplitPdfWorkflow(previewPath: previewPath));
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('By maximum file size'));
    await tester.tap(find.text('By maximum file size'));
    await tester.pumpAndSettle();

    final input = tester.widget<TextField>(
      find.byKey(const ValueKey('split-maximum-size-input')),
    );
    expect(input.controller!.text, '10');
    expect(find.textContaining('Maximum 10 MB per PDF'), findsOneWidget);
    expect(
      find.textContaining('Parts will be determined during splitting.'),
      findsOneWidget,
    );

    await tester.enterText(
      find.byKey(const ValueKey('split-maximum-size-input')),
      '25',
    );
    await tester.pump();
    expect(
      find.text('This PDF is already below the 25 MB limit.'),
      findsOneWidget,
    );
  });

  testWidgets('submits typed mode and displays structured success parts', (
    tester,
  ) async {
    final workflow = FakeSplitPdfWorkflow(previewPath: previewPath);
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Every N pages'));
    await tester.tap(find.text('Every N pages'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('split-every-n-input')),
      '10',
    );
    await tester.ensureVisible(find.byKey(const ValueKey('split-pdf-action')));
    await tester.tap(find.byKey(const ValueKey('split-pdf-action')));
    await tester.pumpAndSettle();

    expect(workflow.submittedMode, SplitPdfMode.everyNPages);
    expect(workflow.submittedEveryN, 10);
    expect(find.byKey(const ValueKey('split-success')), findsOneWidget);
    expect(find.text('Split completed'), findsOneWidget);
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('split-success')),
        matching: find.textContaining('Pages 11–20'),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('split-success')),
        matching: find.textContaining('8.7 MB'),
      ),
      findsOneWidget,
    );
    expect(find.text(r'E:\exports\report-split'), findsOneWidget);
  });
}

Future<void> _pumpPanel(
  WidgetTester tester,
  FakeSplitPdfWorkflow workflow,
) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(1280, 820);
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(body: SplitPdfPanel(workflow: workflow)),
    ),
  );
}
