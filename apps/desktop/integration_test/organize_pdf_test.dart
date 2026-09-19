import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_error;
import 'package:ilikepdf/src/rust/api/organize_pdf.dart' as organize;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as preview;
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets(
    'bridge organizes one multi-source plan with rotation and collision safety',
    (tester) async {
      final sourceDirectory = await Directory.systemTemp.createTemp(
        'ilikepdf-organize-sources-',
      );
      final destination = await Directory.systemTemp.createTemp(
        'ilikepdf-organize-output-',
      );
      addTearDown(() async {
        await sourceDirectory.delete(recursive: true);
        await destination.delete(recursive: true);
      });
      final first = await File(
        '${sourceDirectory.path}${Platform.pathSeparator}A สองหน้า.pdf',
      ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());
      final second = await File(
        '${sourceDirectory.path}${Platform.pathSeparator}B one page.pdf',
      ).writeAsBytes(await _fixture('one_page.pdf').readAsBytes());
      final third = await File(
        '${sourceDirectory.path}${Platform.pathSeparator}C copy.pdf',
      ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());
      final sourceBytes = await Future.wait(
        [first, second, third].map((file) => file.readAsBytes()),
      );

      final initial = await organize.inspectOrganizePdfSources(
        existingSourcePaths: const [],
        candidateSourcePaths: [first.path, second.path],
      );
      final later = await organize.inspectOrganizePdfSources(
        existingSourcePaths: [first.path, second.path],
        candidateSourcePaths: [third.path],
      );
      expect(initial.map((source) => source.pageCount), [2, 1]);
      expect(later.single.pageCount, 2);
      await expectLater(
        organize.inspectOrganizePdfSources(
          existingSourcePaths: [first.path, second.path, third.path],
          candidateSourcePaths: [first.path],
        ),
        throwsA(
          isA<rust_error.ApplicationError>().having(
            (error) => error.message,
            'message',
            contains('already been added'),
          ),
        ),
      );

      final sources = [
        organize.OrganizePdfSource(
          sourceId: 0,
          sourcePath: first.path,
          pageCount: 2,
          hasWarnings: false,
        ),
        organize.OrganizePdfSource(
          sourceId: 1,
          sourcePath: second.path,
          pageCount: 1,
          hasWarnings: false,
        ),
        organize.OrganizePdfSource(
          sourceId: 2,
          sourcePath: third.path,
          pageCount: 2,
          hasWarnings: false,
        ),
      ];
      final pages = [
        const organize.OrganizePdfPageItem(
          pageItemId: 1,
          sourceId: 0,
          sourcePageIndex: 1,
          rotation: organize.OrganizePdfPageRotation.none,
        ),
        const organize.OrganizePdfPageItem(
          pageItemId: 2,
          sourceId: 1,
          sourcePageIndex: 0,
          rotation: organize.OrganizePdfPageRotation.none,
        ),
        const organize.OrganizePdfPageItem(
          pageItemId: 0,
          sourceId: 0,
          sourcePageIndex: 0,
          rotation: organize.OrganizePdfPageRotation.counterClockwise90,
        ),
        const organize.OrganizePdfPageItem(
          pageItemId: 3,
          sourceId: 2,
          sourcePageIndex: 0,
          rotation: organize.OrganizePdfPageRotation.none,
        ),
      ];
      final request = organize.OrganizePdfRequest(
        sources: sources,
        pageItems: pages,
        destinationDirectory: destination.path,
        outputName: 'รายงาน organized',
      );

      final firstUpdates = await organize
          .organizePdf(request: request)
          .toList();
      final secondUpdates = await organize
          .organizePdf(request: request)
          .toList();

      expect(firstUpdates.first.stage, organize.OrganizePdfStage.preparing);
      expect(
        firstUpdates.map((update) => update.stage),
        containsAll([
          organize.OrganizePdfStage.organizing,
          organize.OrganizePdfStage.validating,
          organize.OrganizePdfStage.publishing,
          organize.OrganizePdfStage.completed,
        ]),
      );
      final completed = firstUpdates.last;
      expect(completed.status, organize.OrganizePdfStatus.complete);
      expect(completed.sourceCount, 3);
      expect(completed.pageCount, 4);
      expect(
        File(completed.outputPath!).uri.pathSegments.last,
        'รายงาน organized.pdf',
      );
      expect(
        File(secondUpdates.last.outputPath!).uri.pathSegments.last,
        'รายงาน organized (1).pdf',
      );
      expect(
        (await preview.openPdfDocument(sourcePath: completed.outputPath!))
            .pageCount,
        4,
      );
      final rendered = await Future.wait(
        List.generate(
          4,
          (pageIndex) => preview.renderPdfPage(
            request: preview.RenderPdfPageRequest(
              sourcePath: completed.outputPath!,
              pageIndex: pageIndex,
              targetWidth: 120,
            ),
          ),
        ),
      );
      expect(rendered.map((page) => page.heightPixels), [180, 80, 180, 80]);
      for (var index = 0; index < 3; index++) {
        expect(
          await [first, second, third][index].readAsBytes(),
          sourceBytes[index],
        );
      }
    },
  );
}

File _fixture(String name) =>
    File('../../crates/ilikepdf_pdf/tests/fixtures/$name').absolute;
