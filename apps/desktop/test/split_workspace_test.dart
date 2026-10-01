import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/shared/page_thumbnail_cache.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';
import 'package:ilikepdf/src/app/split_pdf/split_page_workspace.dart';
import 'package:ilikepdf/src/app/split_pdf/split_pdf_panel.dart';
import 'package:ilikepdf/src/app/split_pdf/split_pdf_workflow.dart';

import 'split_pdf_test.dart' show FakeSplitPdfWorkflow;

void main() {
  late Directory directory;
  late FakeSplitPdfWorkflow workflow;
  setUp(() {
    directory = Directory.systemTemp.createTempSync('split-visual-test-');
    final image = File('${directory.path}/preview.png')
      ..writeAsBytesSync(
        base64Decode(
          'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
        ),
      );
    workflow = FakeSplitPdfWorkflow(previewPath: image.path)
      ..selected = _source(10);
  });
  tearDown(() => directory.deleteSync(recursive: true));

  Future<void> load(
    WidgetTester tester, {
    Size size = const Size(1280, 1000),
  }) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = size;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: SplitPdfPanel(workflow: workflow)),
      ),
    );
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
  }

  Future<void> mode(WidgetTester tester, SplitPdfMode mode) async {
    final chip = find.byKey(ValueKey('split-mode-${mode.name}'));
    await tester.ensureVisible(chip);
    await tester.tap(chip);
    await tester.pumpAndSettle();
  }

  SplitPageWorkspace workspace(WidgetTester tester) =>
      tester.widget(find.byType(SplitPageWorkspace));
  List<(int, int)> ranges(WidgetTester tester) =>
      workspace(tester).ranges!
          .map((range) => (range.firstPage, range.lastPage))
          .toList();
  String text(WidgetTester tester) => tester
      .widget<TextField>(find.byKey(const ValueKey('split-after-input')))
      .controller!
      .text;
  bool enabled(WidgetTester tester) =>
      tester
          .widget<PrimaryToolAction>(
            find.byKey(const ValueKey('split-pdf-action')),
          )
          .onPressed !=
      null;

  Future<void> clickPage(WidgetTester tester, int page) async {
    final card = find.byKey(ValueKey('split-page-$page'));
    await tester.scrollUntilVisible(
      card,
      180,
      scrollable: find.descendant(
        of: find.byKey(const ValueKey('split-page-workspace')),
        matching: find.byType(Scrollable),
      ),
    );
    await tester.tap(card);
    await tester.pumpAndSettle();
  }

  testWidgets(
    'every page has a numbered thumbnail and an individual PDF label',
    (tester) async {
      await load(tester);
      expect(find.byType(LazyPageThumbnail), findsWidgets);
      expect(find.text('Part 1 · 1 PDF'), findsOneWidget);
      expect(find.text('Page 1'), findsOneWidget);
      expect(ranges(tester), [
        for (var page = 1; page <= 10; page++) (page, page),
      ]);
      await clickPage(tester, 1);
      expect(ranges(tester).length, 10);
      expect(workflow.submittedMode, isNull);
    },
  );

  testWidgets('every N uses typed groups and reuses cached source thumbnails', (
    tester,
  ) async {
    await load(tester);
    final cache = workspace(tester).cache;
    await mode(tester, SplitPdfMode.everyNPages);
    await tester.enterText(
      find.byKey(const ValueKey('split-every-n-input')),
      '3',
    );
    await tester.pumpAndSettle();
    expect(ranges(tester), [(1, 3), (4, 6), (7, 9), (10, 10)]);
    expect(find.text('Part 1 · Pages 1–3'), findsOneWidget);
    await tester.enterText(
      find.byKey(const ValueKey('split-every-n-input')),
      '2',
    );
    await tester.pumpAndSettle();
    expect(ranges(tester), [(1, 2), (3, 4), (5, 6), (7, 8), (9, 10)]);
    expect(identical(workspace(tester).cache, cache), isTrue);
    expect(workflow.renderedPages.where((page) => page == 0).length, 1);
    expect(workflow.submittedMode, isNull);
  });

  testWidgets(
    'clicking boundaries and typing share one plan, including page one',
    (tester) async {
      workflow.selected = _source(5);
      await load(tester);
      await mode(tester, SplitPdfMode.splitAfterPages);
      await clickPage(tester, 2);
      expect(text(tester), '2');
      await clickPage(tester, 4);
      expect(text(tester), '2,4');
      expect(ranges(tester), [(1, 2), (3, 4), (5, 5)]);
      await clickPage(tester, 2);
      expect(text(tester), '4');
      await clickPage(tester, 1);
      expect(text(tester), '1,4');
      expect(ranges(tester).first, (1, 1));
      await tester.enterText(
        find.byKey(const ValueKey('split-after-input')),
        '2,3',
      );
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('split-boundary-2')), findsOneWidget);
      expect(find.byKey(const ValueKey('split-boundary-1')), findsNothing);
      expect(ranges(tester), [(1, 2), (3, 3), (4, 5)]);
      await tester.tap(find.byKey(const ValueKey('split-pdf-action')));
      await tester.pumpAndSettle();
      expect(workflow.submittedSplitAfter, '2,3');
    },
  );

  testWidgets(
    'final page is disabled and removing the only boundary returns to empty',
    (tester) async {
      workflow.selected = _source(5);
      await load(tester);
      await mode(tester, SplitPdfMode.splitAfterPages);
      await clickPage(tester, 5);
      expect(text(tester), '');
      expect(
        find.byTooltip('Final page — no split boundary needed'),
        findsOneWidget,
      );
      await clickPage(tester, 1);
      expect(text(tester), '1');
      await clickPage(tester, 1);
      expect(text(tester), '');
      expect(workspace(tester).ranges, isNull);
      expect(enabled(tester), isFalse);
    },
  );

  testWidgets(
    'invalid raw text clears groups and disables execution and boundary edits',
    (tester) async {
      await load(tester);
      await mode(tester, SplitPdfMode.splitAfterPages);
      for (final value in ['3,7', '3,', '7,3', '10']) {
        await tester.enterText(
          find.byKey(const ValueKey('split-after-input')),
          value,
        );
        await tester.pumpAndSettle();
        if (value == '3,7') {
          expect(ranges(tester), [(1, 3), (4, 7), (8, 10)]);
        } else {
          expect(workspace(tester).ranges, isNull);
          expect(enabled(tester), isFalse);
          expect(
            find.byKey(const ValueKey('split-configuration-error')),
            findsOneWidget,
          );
          await clickPage(tester, 1);
          expect(text(tester), value);
        }
      }
    },
  );

  testWidgets('late preview responses cannot replace newer text or mode', (
    tester,
  ) async {
    await load(tester);
    await mode(tester, SplitPdfMode.everyNPages);
    final older = Completer<List<SplitPdfRange>?>();
    final newer = Completer<List<SplitPdfRange>?>();
    workflow.previewOverride = (every, _) =>
        every == 3 ? older.future : newer.future;
    await tester.enterText(
      find.byKey(const ValueKey('split-every-n-input')),
      '3',
    );
    await tester.pump();
    expect(enabled(tester), isFalse);
    await tester.enterText(
      find.byKey(const ValueKey('split-every-n-input')),
      '4',
    );
    newer.complete(const [
      SplitPdfRange(firstPage: 1, lastPage: 4),
      SplitPdfRange(firstPage: 5, lastPage: 10),
    ]);
    await tester.pumpAndSettle();
    expect(ranges(tester), [(1, 4), (5, 10)]);
    await mode(tester, SplitPdfMode.maximumFileSize);
    older.complete(const [
      SplitPdfRange(firstPage: 1, lastPage: 3),
      SplitPdfRange(firstPage: 4, lastPage: 10),
    ]);
    await tester.pumpAndSettle();
    expect(workspace(tester).ranges, isNull);
  });

  testWidgets(
    'size mode has no groups until real results and does not rerender for results',
    (tester) async {
      await load(tester);
      await mode(tester, SplitPdfMode.maximumFileSize);
      expect(workspace(tester).ranges, isNull);
      expect(
        find.text('Parts will be determined during splitting.'),
        findsOneWidget,
      );
      await clickPage(tester, 1);
      expect(workspace(tester).ranges, isNull);
      await tester.tap(find.byKey(const ValueKey('split-pdf-action')));
      await tester.pumpAndSettle();
      expect(workspace(tester).partSizes, [8700000, 9800000]);
      expect(
        find.textContaining('Part 1 · Pages 1–10 · 8.7 MB'),
        findsOneWidget,
      );
      expect(workflow.renderedPages.where((page) => page == 0).length, 1);
      await mode(tester, SplitPdfMode.everyPage);
      expect(workspace(tester).compactParts, isTrue);
      expect(workspace(tester).boundaryMode, isFalse);
    },
  );

  testWidgets(
    'large parts virtualize their interior and narrow layout stays usable',
    (tester) async {
      workflow.selected = _source(10000);
      await load(tester, size: const Size(850, 900));
      expect(workflow.renderedPages.length, lessThan(20));
      await mode(tester, SplitPdfMode.everyNPages);
      await tester.enterText(
        find.byKey(const ValueKey('split-every-n-input')),
        '9999',
      );
      await tester.pumpAndSettle();
      expect(ranges(tester), [(1, 9999), (10000, 10000)]);
      expect(find.byType(LazyPageThumbnail).evaluate().length, lessThan(20));
      final target = find.byKey(const ValueKey('split-page-50'));
      await tester.ensureVisible(
        find.byKey(const ValueKey('split-page-workspace')),
      );
      await tester.pumpAndSettle();
      await tester.scrollUntilVisible(
        target,
        240,
        scrollable: find.descendant(
          of: find.byKey(const ValueKey('split-page-workspace')),
          matching: find.byType(Scrollable),
        ),
      );
      await tester.pumpAndSettle();
      expect(target, findsOneWidget);
      expect(find.textContaining('continued'), findsWidgets);
      expect(workflow.renderedPages.length, lessThan(100));
      expect(workspace(tester).cache.cachedCount, lessThanOrEqualTo(32));
      expect(tester.takeException(), isNull);
    },
  );
}

SelectedSplitPdf _source(int pages) => SelectedSplitPdf(
  displayName: 'fixture.pdf',
  sourcePath: r'C:\fixtures\fixture.pdf',
  sourceDirectory: r'C:\fixtures',
  pageCount: pages,
  sizeBytes: 25000000,
  hasWarnings: false,
);
