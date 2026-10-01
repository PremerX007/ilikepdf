import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/organize_pdf/organize_pdf_panel.dart';
import 'package:ilikepdf/src/app/organize_pdf/organize_pdf_workflow.dart';
import 'package:ilikepdf/src/app/organize_pdf/organize_source_identity.dart';
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
const fourthPdf = SelectedOrganizePdf(
  displayName: 'D.pdf',
  sourcePath: r'E:\later\D.pdf',
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

  test(
    'large sessions extend the palette with distinct accent combinations',
    () {
      final identities = List.generate(
        180,
        (index) => OrganizeSourceAccent(index).colors,
      );
      for (var index = 0; index < identities.length; index++) {
        for (var other = 0; other < index; other++) {
          expect(identities[index], isNot(orderedEquals(identities[other])));
        }
        expect(identities[index], OrganizeSourceAccent(index).colors);
      }
    },
  );

  testWidgets('source identities match cards and survive session edits', (
    tester,
  ) async {
    final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
      ..selections.addAll([
        [firstPdf, secondPdf, thirdPdf],
        [fourthPdf],
        [secondPdf],
      ]);
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();

    final first = _identity(tester, 'organize-source-identity-0').accent;
    final second = _identity(tester, 'organize-source-identity-1').accent;
    final third = _identity(tester, 'organize-source-identity-2').accent;
    expect([first.index, second.index, third.index], [0, 1, 2]);
    expect({
      first.primaryColor,
      second.primaryColor,
      third.primaryColor,
    }, hasLength(3));
    expect(_identity(tester, 'organize-page-identity-0').accent, same(first));
    expect(_identity(tester, 'organize-page-identity-1').accent, same(first));
    expect(_identity(tester, 'organize-page-identity-2').accent, same(second));
    expect(_identity(tester, 'organize-page-identity-3').accent, same(third));

    await tester.tap(find.byKey(const ValueKey('add-organize-pdfs')));
    await tester.pumpAndSettle();
    expect(_identity(tester, 'organize-page-identity-0').accent, same(first));
    expect(_identity(tester, 'organize-page-identity-2').accent, same(second));
    expect(_identity(tester, 'organize-page-identity-3').accent, same(third));
    final fourth = _identity(tester, 'organize-source-identity-3').accent;
    expect(fourth.index, 3);
    expect(_identity(tester, 'organize-page-identity-4').accent, same(fourth));

    await tester.tap(find.byKey(const ValueKey('remove-organize-source-1')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('organize-page-card-2')), findsNothing);
    expect(_identity(tester, 'organize-source-identity-0').accent, same(first));
    expect(_identity(tester, 'organize-page-identity-3').accent, same(third));
    expect(_identity(tester, 'organize-page-identity-4').accent, same(fourth));

    await tester.tap(find.byKey(const ValueKey('rotate-right-0')));
    await tester.tap(find.byKey(const ValueKey('delete-organize-page-1')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('reset-organize-all')));
    await tester.pumpAndSettle();
    expect(_cardOrder(tester), [0, 1, 3, 4]);
    expect(_identity(tester, 'organize-page-identity-0').accent, same(first));
    expect(_identity(tester, 'organize-page-identity-1').accent, same(first));
    expect(_identity(tester, 'organize-page-identity-3').accent, same(third));
    expect(_identity(tester, 'organize-page-identity-4').accent, same(fourth));
    expect(_rotation(tester, 0), 0);

    await tester.tap(find.byKey(const ValueKey('add-organize-pdfs')));
    await tester.pumpAndSettle();
    expect(_identity(tester, 'organize-source-identity-4').accent.index, 4);
    expect(_identity(tester, 'organize-page-identity-5').accent.index, 4);
    expect(_identity(tester, 'organize-page-identity-3').accent, same(third));
  });

  for (final brightness in Brightness.values) {
    testWidgets(
      'source backgrounds stay distinct under a red $brightness theme',
      (tester) async {
        final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
          ..selections.add([firstPdf, secondPdf]);
        await _pumpPanel(
          tester,
          workflow,
          theme: ThemeData(
            colorScheme: ColorScheme.fromSeed(
              seedColor: const Color(0xFFB42318),
              brightness: brightness,
            ),
          ),
        );
        await tester.tap(find.text('Select PDFs'));
        await tester.pumpAndSettle();

        Card card(int id) => tester.widget<Card>(
          find.descendant(
            of: find.byKey(ValueKey('organize-page-card-$id')),
            matching: find.byType(Card),
          ),
        );
        Color preview(int id) => tester
            .widget<ColoredBox>(
              find.byKey(ValueKey('organize-page-preview-surface-$id')),
            )
            .color;
        Color? sourceSurface(int id) =>
            (tester
                        .widget<DecoratedBox>(
                          find.byKey(ValueKey('organize-source-surface-$id')),
                        )
                        .decoration
                    as BoxDecoration)
                .color;

        final firstBackground = card(0).color;
        final secondBackground = card(2).color;
        final firstPreview = preview(0);
        final secondPreview = preview(2);
        expect(firstBackground, isNotNull);
        expect(firstBackground, isNot(secondBackground));
        expect(card(1).color, firstBackground);
        expect(firstPreview, isNot(secondPreview));
        expect(preview(1), firstPreview);
        expect(sourceSurface(0), firstBackground);
        expect(sourceSurface(1), secondBackground);
        expect(card(0).surfaceTintColor, Colors.transparent);
        expect(card(2).surfaceTintColor, Colors.transparent);

        // A different app seed must not recolor source-owned surfaces.
        await tester.pumpWidget(
          MaterialApp(
            theme: ThemeData(
              colorScheme: ColorScheme.fromSeed(
                seedColor: Colors.green,
                brightness: brightness,
              ),
            ),
            home: Scaffold(body: OrganizePdfPanel(workflow: workflow)),
          ),
        );
        await tester.pumpAndSettle();
        expect(card(0).color, firstBackground);
        expect(card(2).color, secondBackground);
        expect(preview(0), firstPreview);
        expect(preview(2), secondPreview);
        expect(sourceSurface(0), firstBackground);
        expect(sourceSurface(1), secondBackground);
      },
    );
  }

  testWidgets('source filename is available in tooltips and semantics', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
        ..selections.add([firstPdf, secondPdf]);
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();
      expect(find.bySemanticsLabel(RegExp(r'Source 1: A\.pdf')), findsWidgets);
      expect(find.bySemanticsLabel(RegExp(r'Source 2: B\.pdf')), findsWidgets);
      expect(find.byTooltip('Source 1: A.pdf'), findsWidgets);
      expect(find.byTooltip('Source 2: B.pdf'), findsWidgets);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('forward drag opens a card slot and Reset restores order', (
    tester,
  ) async {
    final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
      ..selections.add([firstPdf, secondPdf]);
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    final gesture = await tester.startGesture(
      tester.getCenter(find.byKey(const ValueKey('organize-page-thumbnail-0'))),
      kind: PointerDeviceKind.mouse,
    );
    await gesture.moveBy(const Offset(24, 0));
    await tester.pump();
    await gesture.moveTo(
      tester.getCenter(find.byKey(const ValueKey('lazy-reorder-target-2'))),
    );
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.byKey(const ValueKey('reorder-placeholder')), findsOne);
    expect(
      tester.getCenter(find.byKey(const ValueKey('reorder-placeholder'))),
      tester.getCenter(find.byKey(const ValueKey('lazy-reorder-target-2'))),
    );
    await gesture.up();
    await tester.pumpAndSettle();
    expect(_cardOrder(tester), [1, 2, 0]);
    await tester.tap(find.byKey(const ValueKey('reset-organize-all')));
    await tester.pumpAndSettle();
    expect(_cardOrder(tester), [0, 1, 2]);
  });

  for (final surface in ['thumbnail', 'label', 'empty area']) {
    testWidgets('mouse drag from $surface reorders across sources', (
      tester,
    ) async {
      final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
        ..selections.add([firstPdf, secondPdf]);
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();
      final card = find.byKey(const ValueKey('organize-page-card-2'));
      final start = switch (surface) {
        'thumbnail' => tester.getCenter(
          find.byKey(const ValueKey('organize-page-thumbnail-2')),
        ),
        'label' => tester.getCenter(
          find.byKey(const ValueKey('organize-page-label-2')),
        ),
        _ => tester.getBottomLeft(card) + const Offset(14, -24),
      };
      final sourceAccent = _identity(tester, 'organize-page-identity-2').accent;
      final gesture = await tester.startGesture(
        start,
        kind: PointerDeviceKind.mouse,
      );
      await gesture.moveBy(const Offset(24, 0));
      await tester.pump();
      final proxy = find.byKey(const ValueKey('reorder-card-proxy'));
      expect(proxy, findsOne);
      expect(
        tester
            .widget<OrganizeSourceIdentity>(
              find.descendant(
                of: proxy,
                matching: find.byType(OrganizeSourceIdentity),
              ),
            )
            .accent,
        same(sourceAccent),
      );
      await gesture.moveTo(
        tester.getCenter(find.byKey(const ValueKey('lazy-reorder-target-0'))),
      );
      await tester.pump();
      expect(find.byKey(const ValueKey('reorder-placeholder')), findsOne);
      await gesture.up();
      await tester.pumpAndSettle();
      expect(_cardOrder(tester), [2, 0, 1]);
      expect(
        _identity(tester, 'organize-page-identity-2').accent,
        same(sourceAccent),
      );

      await tester.tap(find.byKey(const ValueKey('organize-pdf-action')));
      await tester.pumpAndSettle();
      expect(workflow.submittedPages.map((page) => page.pageItemId), [2, 0, 1]);
      expect(workflow.submittedPages.map((page) => page.sourceId), [1, 0, 0]);
    });
  }

  testWidgets('card clicks and secondary mouse drags do not reorder', (
    tester,
  ) async {
    final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
      ..selections.add([firstPdf, secondPdf]);
    await _pumpPanel(tester, workflow);
    await tester.tap(find.text('Select PDFs'));
    await tester.pumpAndSettle();
    final start = tester.getCenter(
      find.byKey(const ValueKey('organize-page-thumbnail-2')),
    );
    final click = await tester.startGesture(
      start,
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byKey(const ValueKey('reorder-card-proxy')), findsNothing);
    await click.up();
    await tester.pumpAndSettle();
    expect(_cardOrder(tester), [0, 1, 2]);

    final secondary = await tester.startGesture(
      start,
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryButton,
    );
    await secondary.moveTo(
      tester.getCenter(find.byKey(const ValueKey('lazy-reorder-target-0'))),
    );
    await tester.pump();
    expect(find.byKey(const ValueKey('reorder-card-proxy')), findsNothing);
    await secondary.up();
    await tester.pumpAndSettle();
    expect(_cardOrder(tester), [0, 1, 2]);
  });

  for (final action in [
    'rotate-left-2',
    'rotate-right-2',
    'delete-organize-page-2',
  ]) {
    testWidgets('$action clicks act once and drags cannot reorder', (
      tester,
    ) async {
      final workflow = FakeOrganizePdfWorkflow(previewPath: previewPath)
        ..selections.add([firstPdf, secondPdf]);
      await _pumpPanel(tester, workflow);
      await tester.tap(find.text('Select PDFs'));
      await tester.pumpAndSettle();
      final button = find.byKey(ValueKey(action));
      final gesture = await tester.startGesture(
        tester.getCenter(button),
        kind: PointerDeviceKind.mouse,
      );
      await gesture.moveTo(
        tester.getCenter(find.byKey(const ValueKey('lazy-reorder-target-0'))),
      );
      await tester.pump();
      expect(find.byKey(const ValueKey('reorder-card-proxy')), findsNothing);
      expect(find.byKey(const ValueKey('reorder-placeholder')), findsNothing);
      await gesture.up();
      await tester.pumpAndSettle();
      expect(_cardOrder(tester), [0, 1, 2]);
      expect(_rotation(tester, 2), 0);

      await tester.tap(button);
      await tester.pumpAndSettle();
      if (action.startsWith('delete')) {
        expect(_cardOrder(tester), [0, 1]);
        expect(find.text('B.pdf'), findsOne);
      } else {
        expect(_cardOrder(tester), [0, 1, 2]);
        expect(_rotation(tester, 2), action.startsWith('rotate-left') ? 3 : 1);
      }
    });
  }

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
      tester.getCenter(find.byKey(const ValueKey('organize-page-thumbnail-2'))),
      kind: PointerDeviceKind.mouse,
    );
    await gesture.moveBy(const Offset(24, 0));
    await tester.pump();
    await gesture.moveTo(
      tester.getCenter(find.byKey(const ValueKey('lazy-reorder-target-0'))),
    );
    await tester.pump(const Duration(milliseconds: 500));
    await gesture.up();
    await tester.pumpAndSettle();
    expect(_cardOrder(tester), [2, 0, 1]);

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

OrganizeSourceIdentity _identity(WidgetTester tester, String key) =>
    tester.widget<OrganizeSourceIdentity>(find.byKey(ValueKey(key)));

int _rotation(WidgetTester tester, int pageId) => tester
    .widget<RotatedBox>(find.byKey(ValueKey('organize-page-rotation-$pageId')))
    .quarterTurns;

List<int> _cardOrder(WidgetTester tester) => tester
    .widgetList(
      find.byWidgetPredicate((widget) {
        final key = widget.key;
        return key is ValueKey<String> &&
            key.value.startsWith('organize-page-card-');
      }),
    )
    .map(
      (widget) =>
          int.parse((widget.key! as ValueKey<String>).value.split('-').last),
    )
    .toList();

Future<void> _pumpPanel(
  WidgetTester tester,
  FakeOrganizePdfWorkflow workflow, {
  ThemeData? theme,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(1280, 820);
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
  await tester.pumpWidget(
    MaterialApp(
      theme: theme,
      home: Scaffold(body: OrganizePdfPanel(workflow: workflow)),
    ),
  );
}

PrimaryToolAction _primaryAction(WidgetTester tester) =>
    tester.widget<PrimaryToolAction>(
      find.byKey(const ValueKey('organize-pdf-action')),
    );
