import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
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
