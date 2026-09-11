import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/rust/api/image_to_pdf.dart' as image_pdf;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as pdf;
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets('uses the first ordered image for merged naming and page order', (
    WidgetTester tester,
  ) async {
    final sources = [
      _fixture('sample.webp'),
      _fixture('portrait.png'),
      _fixture('photo.jpg'),
    ];
    final before = await Future.wait(
      sources.map((source) => source.readAsBytes()),
    );
    final destination = await Directory.systemTemp.createTemp(
      'ilikepdf-image-pdf-merged-',
    );
    addTearDown(() => destination.delete(recursive: true));

    final updates = await image_pdf
        .createPdfsFromImages(
          request: image_pdf.CreateImagePdfRequest(
            sourcePaths: sources.map((source) => source.path).toList(),
            destinationDirectory: destination.path,
            pageSize: image_pdf.ImagePdfPageSize.a4,
            orientation: image_pdf.ImagePdfOrientation.landscape,
            margin: image_pdf.ImagePdfMargin.small,
            merge: true,
          ),
        )
        .toList();

    expect(updates.first.status, image_pdf.ImagePdfStatus.running);
    expect(updates.last.status, image_pdf.ImagePdfStatus.complete);
    expect(updates.last.completedImageCount, 3);
    expect(updates.last.outputFiles.length, 1);
    expect(
      File(updates.last.outputFiles.single).uri.pathSegments.last,
      'sample.pdf',
    );

    final info = await pdf.openPdfDocument(
      sourcePath: updates.last.outputFiles.single,
    );
    expect(info.pageCount, 3);
    expect(info.firstPageSize?.widthPoints, closeTo(841.8898, 0.01));
    expect(info.firstPageSize?.heightPoints, closeTo(595.2756, 0.01));
    for (var index = 0; index < sources.length; index += 1) {
      final rendered = await pdf.renderPdfPage(
        request: pdf.RenderPdfPageRequest(
          sourcePath: updates.last.outputFiles.single,
          pageIndex: index,
          targetWidth: 240,
          destinationPath: null,
        ),
      );
      final preview = File(rendered.outputPath);
      expect(await preview.exists(), isTrue);
      await preview.delete();
      expect(await sources[index].readAsBytes(), before[index]);
    }
  });

  testWidgets('creates three separate one-page PDFs', (
    WidgetTester tester,
  ) async {
    final sources = [
      _fixture('portrait.png'),
      _fixture('photo.jpg'),
      _fixture('sample.webp'),
    ];
    final destination = await Directory.systemTemp.createTemp(
      'ilikepdf-image-pdf-separate-',
    );
    addTearDown(() => destination.delete(recursive: true));
    final existing = File('${destination.path}\\portrait.pdf');
    await existing.writeAsBytes(const [1, 2, 3, 4]);

    final updates = await image_pdf
        .createPdfsFromImages(
          request: image_pdf.CreateImagePdfRequest(
            sourcePaths: sources.map((source) => source.path).toList(),
            destinationDirectory: destination.path,
            pageSize: image_pdf.ImagePdfPageSize.fit,
            orientation: image_pdf.ImagePdfOrientation.portrait,
            margin: image_pdf.ImagePdfMargin.big,
            merge: false,
          ),
        )
        .toList();

    final completed = updates.last;
    expect(completed.status, image_pdf.ImagePdfStatus.complete);
    expect(
      completed.outputFiles.map((path) => File(path).uri.pathSegments.last),
      ['portrait (1).pdf', 'photo.pdf', 'sample.pdf'],
    );
    expect(await existing.readAsBytes(), const [1, 2, 3, 4]);
    for (final output in completed.outputFiles) {
      expect((await pdf.openPdfDocument(sourcePath: output)).pageCount, 1);
    }
  });

  testWidgets('honors EXIF orientation and numbers repeated merged outputs', (
    WidgetTester tester,
  ) async {
    final destination = await Directory.systemTemp.createTemp(
      'ilikepdf-image-pdf-orientation-',
    );
    addTearDown(() => destination.delete(recursive: true));
    final source = _fixture('oriented.jpg');
    final request = image_pdf.CreateImagePdfRequest(
      sourcePaths: [source.path],
      destinationDirectory: destination.path,
      pageSize: image_pdf.ImagePdfPageSize.fit,
      orientation: image_pdf.ImagePdfOrientation.landscape,
      margin: image_pdf.ImagePdfMargin.big,
      merge: true,
    );

    final first = await image_pdf
        .createPdfsFromImages(request: request)
        .toList();
    expect(first.last.status, image_pdf.ImagePdfStatus.complete);
    final info = await pdf.openPdfDocument(
      sourcePath: first.last.outputFiles.single,
    );
    expect(info.firstPageSize?.widthPoints, closeTo(60, 0.01));
    expect(info.firstPageSize?.heightPoints, closeTo(90, 0.01));

    final second = await image_pdf
        .createPdfsFromImages(request: request)
        .toList();
    expect(second.last.status, image_pdf.ImagePdfStatus.complete);
    expect(
      File(second.last.outputFiles.single).uri.pathSegments.last,
      'oriented (1).pdf',
    );
    final firstBytes = await File(first.last.outputFiles.single).readAsBytes();
    final secondBytes = await File(second.last.outputFiles.single)
        .readAsBytes();

    final third = await image_pdf
        .createPdfsFromImages(request: request)
        .toList();
    expect(third.last.status, image_pdf.ImagePdfStatus.complete);
    expect(
      File(third.last.outputFiles.single).uri.pathSegments.last,
      'oriented (2).pdf',
    );
    expect(await File(first.last.outputFiles.single).readAsBytes(), firstBytes);
    expect(
      await File(second.last.outputFiles.single).readAsBytes(),
      secondBytes,
    );
  });
}

File _fixture(String name) =>
    File('../../crates/ilikepdf_pdf/tests/fixtures/images/$name').absolute;
