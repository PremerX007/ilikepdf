import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/split_pdf/split_page_workspace.dart';
import 'package:ilikepdf/src/app/split_pdf/split_pdf_panel.dart';
import 'package:ilikepdf/src/app/split_pdf/split_pdf_workflow.dart';
import 'package:ilikepdf/src/rust/api/error.dart';
import 'package:ilikepdf/src/rust/api/merge_pdf.dart' as merge;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as preview;
import 'package:ilikepdf/src/rust/api/split_pdf.dart' as split;
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets(
    'bridge structurally splits one Unicode source and numbers folder collisions',
    (tester) async {
      final sources = await Directory.systemTemp.createTemp(
        'ilikepdf-split-sources-',
      );
      final destination = await Directory.systemTemp.createTemp(
        'ilikepdf-split-output-',
      );
      addTearDown(() async {
        await sources.delete(recursive: true);
        await destination.delete(recursive: true);
      });
      final first = await File(
        '${sources.path}${Platform.pathSeparator}A two pages.pdf',
      ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());
      final second = await File(
        '${sources.path}${Platform.pathSeparator}B one page.pdf',
      ).writeAsBytes(await _fixture('one_page.pdf').readAsBytes());
      final merged =
          (await merge
                  .mergePdf(
                    request: merge.MergePdfRequest(
                      sourcePaths: [first.path, second.path, first.path],
                      destinationDirectory: sources.path,
                      outputName: 'รายงาน ผสม.pdf',
                    ),
                  )
                  .toList())
              .last;
      expect(merged.status, merge.MergePdfStatus.complete);
      final source = File(merged.outputPath!);
      final sourceBefore = await source.readAsBytes();
      final request = split.SplitPdfRequest(
        sourcePath: source.path,
        destinationDirectory: destination.path,
        mode: split.SplitPdfMode.everyNPages,
        everyNPages: 2,
      );

      final firstUpdates = await split.splitPdf(request: request).toList();
      final secondUpdates = await split.splitPdf(request: request).toList();

      expect(firstUpdates.first.stage, split.SplitPdfStage.preparing);
      expect(
        firstUpdates.any(
          (update) =>
              update.status == split.SplitPdfStatus.running &&
              update.stage == split.SplitPdfStage.creating,
        ),
        isTrue,
      );
      expect(
        firstUpdates.any(
          (update) => update.stage == split.SplitPdfStage.validating,
        ),
        isTrue,
      );
      final completed = firstUpdates.last;
      final planned = await split.previewSplitPdfRanges(
        pageCount: 5,
        mode: split.SplitPdfMode.everyNPages,
        everyNPages: 2,
      );
      expect(
        planned!.map((range) => (range.firstPage, range.lastPage)),
        completed.parts.map((part) => (part.firstPage, part.lastPage)),
      );
      expect(completed.status, split.SplitPdfStatus.complete);
      expect(completed.sourcePageCount, 5);
      expect(completed.parts.map((part) => (part.firstPage, part.lastPage)), [
        (1, 2),
        (3, 4),
        (5, 5),
      ]);
      expect(
        completed.parts.map(
          (part) => File(part.outputPath).uri.pathSegments.last,
        ),
        [
          'รายงาน ผสม-part-0001.pdf',
          'รายงาน ผสม-part-0002.pdf',
          'รายงาน ผสม-part-0003.pdf',
        ],
      );
      for (var index = 0; index < completed.parts.length; index++) {
        final info = await preview.openPdfDocument(
          sourcePath: completed.parts[index].outputPath,
        );
        expect(info.pageCount, [2, 2, 1][index]);
      }
      expect(
        Directory(completed.outputDirectory!).uri.pathSegments
            .where((segment) => segment.isNotEmpty)
            .last,
        'รายงาน ผสม-split',
      );
      expect(
        Directory(secondUpdates.last.outputDirectory!).uri.pathSegments
            .where((segment) => segment.isNotEmpty)
            .last,
        'รายงาน ผสม-split (1)',
      );
      expect(await source.readAsBytes(), sourceBefore);

      // All deterministic modes share the bridge preview and execution policy.
      for (final mode in [
        split.SplitPdfMode.everyPage,
        split.SplitPdfMode.splitAfterPages,
      ]) {
        final ranges = await split.previewSplitPdfRanges(
          pageCount: 5,
          mode: mode,
          splitAfterPages: '1,3',
        );
        final result =
            (await split
                    .splitPdf(
                      request: split.SplitPdfRequest(
                        sourcePath: source.path,
                        destinationDirectory: destination.path,
                        mode: mode,
                        splitAfterPages: '1,3',
                      ),
                    )
                    .toList())
                .last;
        expect(result.status, split.SplitPdfStatus.complete);
        expect(
          result.parts.map((part) => (part.firstPage, part.lastPage)),
          ranges!.map((range) => (range.firstPage, range.lastPage)),
        );
      }
      expect(
        await split.previewSplitPdfRanges(
          pageCount: 5,
          mode: split.SplitPdfMode.maximumFileSize,
        ),
        isNull,
      );
      for (final invalid in ['3,', '3,1', '5']) {
        await expectLater(
          split.previewSplitPdfRanges(
            pageCount: 5,
            mode: split.SplitPdfMode.splitAfterPages,
            splitAfterPages: invalid,
          ),
          throwsA(isA<ApplicationError>()),
        );
      }
      expect(await source.readAsBytes(), sourceBefore);
    },
  );

  testWidgets(
    'Windows visual workspace synchronizes boundaries and executes the displayed plan',
    (tester) async {
      final directory = await Directory.systemTemp.createTemp(
        'split-workspace-native-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final merged =
          (await merge
                  .mergePdf(
                    request: merge.MergePdfRequest(
                      sourcePaths: [
                        _fixture('two_page.pdf').path,
                        _fixture('two_page.pdf').path,
                        _fixture('one_page.pdf').path,
                      ],
                      destinationDirectory: directory.path,
                      outputName: 'five-pages.pdf',
                    ),
                  )
                  .toList())
              .last;
      expect(merged.status, merge.MergePdfStatus.complete);
      final source = File(merged.outputPath!);
      final before = await source.readAsBytes();
      final workflow = _VisualSplitFixture(source.path);
      final captureKey = GlobalKey();
      await tester.binding.setSurfaceSize(const Size(1280, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        MaterialApp(
          theme: ThemeData(
            useMaterial3: true,
            colorScheme: ColorScheme.fromSeed(
              seedColor: const Color(0xFFB42318),
              surface: const Color(0xFFFFFBFA),
            ),
          ),
          home: RepaintBoundary(
            key: captureKey,
            child: Scaffold(body: SplitPdfPanel(workflow: workflow)),
          ),
        ),
      );
      await tester.tap(find.text('Select PDF'));
      await _waitFor(
        tester,
        () => find
            .byKey(const ValueKey('page-thumbnail-4'))
            .evaluate()
            .isNotEmpty,
      );
      expect(find.text('Part 1 · 1 PDF'), findsOneWidget);
      expect(find.text('Part 5 · 1 PDF'), findsOneWidget);
      await _capture(tester, captureKey, 'every-page');

      Future<void> mode(SplitPdfMode mode) async {
        await tester.tap(find.byKey(ValueKey('split-mode-${mode.name}')));
        await tester.pumpAndSettle();
      }

      SplitPageWorkspace workspace() =>
          tester.widget(find.byType(SplitPageWorkspace));
      Future<void> waitForRanges(List<(int, int)> expected) async {
        await _waitFor(
          tester,
          () =>
              workspace().ranges != null &&
              workspace().ranges!
                      .map((range) => (range.firstPage, range.lastPage))
                      .toList()
                      .toString() ==
                  expected.toString(),
        );
        expect(
          workspace().ranges!.map((range) => (range.firstPage, range.lastPage)),
          expected,
        );
      }

      Future<void> click(int page) async {
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

      String text() => tester
          .widget<TextField>(find.byKey(const ValueKey('split-after-input')))
          .controller!
          .text;

      await mode(SplitPdfMode.everyNPages);
      await waitForRanges([(1, 2), (3, 4), (5, 5)]);
      await _capture(tester, captureKey, 'every-two');
      await tester.enterText(
        find.byKey(const ValueKey('split-every-n-input')),
        '3',
      );
      await waitForRanges([(1, 3), (4, 5)]);
      await mode(SplitPdfMode.splitAfterPages);
      await click(2);
      await waitForRanges([(1, 2), (3, 5)]);
      expect(text(), '2');
      await click(4);
      await waitForRanges([(1, 2), (3, 4), (5, 5)]);
      expect(text(), '2,4');
      await _capture(tester, captureKey, 'split-after');
      await click(2);
      await waitForRanges([(1, 4), (5, 5)]);
      expect(text(), '4');
      await tester.enterText(
        find.byKey(const ValueKey('split-after-input')),
        '1,3',
      );
      await waitForRanges([(1, 1), (2, 3), (4, 5)]);
      await click(5);
      expect(text(), '1,3');
      await mode(SplitPdfMode.maximumFileSize);
      expect(workspace().ranges, isNull);
      expect(
        find.text('Parts will be determined during splitting.'),
        findsOneWidget,
      );
      await _capture(tester, captureKey, 'maximum-size');
      await mode(SplitPdfMode.splitAfterPages);
      await waitForRanges([(1, 1), (2, 3), (4, 5)]);
      await tester.tap(find.byKey(const ValueKey('split-pdf-action')));
      await _waitFor(
        tester,
        () => find.byKey(const ValueKey('split-success')).evaluate().isNotEmpty,
      );
      expect(
        workspace().ranges!.map((range) => (range.firstPage, range.lastPage)),
        [(1, 1), (2, 3), (4, 5)],
      );
      expect(await source.readAsBytes(), before);
      final output = Directory('${directory.path}/five-pages-split');
      final files = await output
          .list()
          .where((file) => file.path.endsWith('.pdf'))
          .toList();
      expect(files.length, 3);
      for (final file in files) {
        final info = await preview.openPdfDocument(sourcePath: file.path);
        expect(info.pageCount, file.path.endsWith('0001.pdf') ? 1 : 2);
      }
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pumpAndSettle();
    },
  );

  testWidgets('bridge rejects a one-page split without publishing', (
    tester,
  ) async {
    final destination = await Directory.systemTemp.createTemp(
      'ilikepdf-split-one-page-',
    );
    addTearDown(() => destination.delete(recursive: true));
    final source = _fixture('one_page.pdf');
    final before = await source.readAsBytes();

    final updates = await split
        .splitPdf(
          request: split.SplitPdfRequest(
            sourcePath: source.path,
            destinationDirectory: destination.path,
            mode: split.SplitPdfMode.everyPage,
          ),
        )
        .toList();

    expect(updates.last.status, split.SplitPdfStatus.failed);
    expect(updates.last.error?.code.name, 'pdfHasTooFewPages');
    expect(await destination.list().isEmpty, isTrue);
    expect(await source.readAsBytes(), before);
  });
}

File _fixture(String name) =>
    File('../../crates/ilikepdf_pdf/tests/fixtures/$name').absolute;

class _VisualSplitFixture extends LocalSplitPdfWorkflow {
  const _VisualSplitFixture(this.sourcePath);
  final String sourcePath;

  @override
  Future<SelectedSplitPdf?> selectPdf() => preparePdfPaths([sourcePath]);
}

Future<void> _waitFor(WidgetTester tester, bool Function() condition) async {
  final deadline = DateTime.now().add(const Duration(seconds: 30));
  while (!condition() && DateTime.now().isBefore(deadline)) {
    await tester.pump(const Duration(milliseconds: 100));
  }
  expect(condition(), isTrue);
  await tester.pumpAndSettle();
}

Future<void> _capture(WidgetTester tester, GlobalKey key, String name) async {
  // Opt-in artifact capture, using only generated public test fixtures.
  const destination = String.fromEnvironment('SPLIT_SCREENSHOT_DIRECTORY');
  if (destination.isEmpty) return;
  await tester.pumpAndSettle();
  final boundary =
      key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
  final image = await boundary.toImage();
  final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
  image.dispose();
  await Directory(destination).create(recursive: true);
  await File('$destination/$name.png')
      .writeAsBytes(bytes!.buffer.asUint8List());
}
