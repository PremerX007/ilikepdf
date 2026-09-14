import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/rust/api/merge_pdf.dart' as merge;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as pdf;
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets(
    'bridge merges ordered duplicate PDFs and numbers repeated outputs',
    (WidgetTester tester) async {
      final sources = await Directory.systemTemp.createTemp(
        'ilikepdf-merge-sources-',
      );
      final destination = await Directory.systemTemp.createTemp(
        'ilikepdf-merge-output-',
      );
      addTearDown(() async {
        await sources.delete(recursive: true);
        await destination.delete(recursive: true);
      });
      final first = await File(
        '${sources.path}${Platform.pathSeparator}A two pages.pdf',
      ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());
      final second = await File(
        '${sources.path}${Platform.pathSeparator}B ภาษาไทย.pdf',
      ).writeAsBytes(await _fixture('one_page.pdf').readAsBytes());
      final before = {
        first.path: await first.readAsBytes(),
        second.path: await second.readAsBytes(),
      };
      final request = merge.MergePdfRequest(
        sourcePaths: [first.path, second.path, first.path],
        destinationDirectory: destination.path,
        outputName: 'merged',
      );

      final firstUpdates = await merge.mergePdf(request: request).toList();
      final secondUpdates = await merge.mergePdf(request: request).toList();

      expect(
        firstUpdates
            .where((update) => update.status == merge.MergePdfStatus.running)
            .map((update) => update.stage),
        [
          merge.MergePdfStage.preparing,
          merge.MergePdfStage.merging,
          merge.MergePdfStage.validating,
          merge.MergePdfStage.publishing,
          merge.MergePdfStage.completed,
        ],
      );
      final completed = firstUpdates.last;
      expect(completed.status, merge.MergePdfStatus.complete);
      expect(completed.inputCount, 3);
      expect(completed.totalPageCount, 5);
      expect(File(completed.outputPath!).uri.pathSegments.last, 'merged.pdf');
      expect(
        File(secondUpdates.last.outputPath!).uri.pathSegments.last,
        'merged (1).pdf',
      );
      expect(
        (await pdf.openPdfDocument(sourcePath: completed.outputPath!))
            .pageCount,
        5,
      );
      for (final pageIndex in [0, 1, 2, 4]) {
        final rendered = await pdf.renderPdfPage(
          request: pdf.RenderPdfPageRequest(
            sourcePath: completed.outputPath!,
            pageIndex: pageIndex,
            targetWidth: 120,
            destinationPath: null,
          ),
        );
        expect(await File(rendered.outputPath).exists(), isTrue);
        await File(rendered.outputPath).delete();
      }
      expect(await first.readAsBytes(), before[first.path]);
      expect(await second.readAsBytes(), before[second.path]);
    },
  );

  testWidgets('bridge reports one malformed input without publishing', (
    WidgetTester tester,
  ) async {
    final destination = await Directory.systemTemp.createTemp(
      'ilikepdf-merge-invalid-',
    );
    addTearDown(() => destination.delete(recursive: true));

    final updates = await merge
        .mergePdf(
          request: merge.MergePdfRequest(
            sourcePaths: [
              _fixture('one_page.pdf').path,
              _fixture('malformed.pdf').path,
            ],
            destinationDirectory: destination.path,
            outputName: 'must-not-exist.pdf',
          ),
        )
        .toList();

    expect(updates.last.status, merge.MergePdfStatus.failed);
    expect(updates.last.failedInputIndex, 1);
    expect(updates.last.error?.code.name, 'invalidPdf');
    expect(
      await File(
        '${destination.path}${Platform.pathSeparator}must-not-exist.pdf',
      ).exists(),
      isFalse,
    );
  });
}

File _fixture(String name) =>
    File('../../crates/ilikepdf_pdf/tests/fixtures/$name').absolute;
