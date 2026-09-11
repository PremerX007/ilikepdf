import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/app.dart';
import 'package:ilikepdf/src/app/image_to_pdf/image_to_pdf_workflow.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';

class FakeImageToPdfWorkflow implements ImageToPdfWorkflow {
  FakeImageToPdfWorkflow({
    required List<List<SelectedImage>> selections,
    this.destination = r'C:\exports',
    this.failure,
  }) : selections = List.of(selections);

  final List<List<SelectedImage>> selections;
  final String destination;
  final ImagePdfProblem? failure;
  List<String>? createdSourcePaths;
  ImagePageSize? createdPageSize;
  ImagePageOrientation? createdOrientation;
  ImagePageMargin? createdMargin;
  bool? createdMerge;
  List<String>? preparedDroppedPaths;

  @override
  Future<String?> chooseDestinationDirectory() async => destination;

  @override
  Future<List<SelectedImage>> prepareImagePaths(
    List<String> sourcePaths,
  ) async {
    preparedDroppedPaths = List.of(sourcePaths);
    const supported = {'jpg', 'jpeg', 'png', 'webp'};
    final images = <SelectedImage>[];
    for (final path in sourcePaths) {
      final filename = path.split(RegExp(r'[\\/]')).last;
      final extension = filename.contains('.')
          ? filename.split('.').last.toLowerCase()
          : '';
      if (!supported.contains(extension)) {
        throw const ImageSelectionException(
          ImagePdfProblem(
            code: ImagePdfProblemCode.unsupportedImageFormat,
            message: 'Only JPG, JPEG, PNG, and WebP images are supported.',
          ),
        );
      }
      final separator = path.lastIndexOf(RegExp(r'[\\/]'));
      images.add(
        SelectedImage(
          displayName: filename,
          sourcePath: path,
          sourceDirectory: separator < 0 ? '.' : path.substring(0, separator),
        ),
      );
    }
    return images;
  }

  @override
  Stream<ImagePdfCreationUpdate> createPdfs({
    required List<String> sourcePaths,
    required String destinationDirectory,
    required ImagePageSize pageSize,
    required ImagePageOrientation orientation,
    required ImagePageMargin margin,
    required bool merge,
  }) async* {
    createdSourcePaths = List.of(sourcePaths);
    createdPageSize = pageSize;
    createdOrientation = orientation;
    createdMargin = margin;
    createdMerge = merge;
    yield ImagePdfCreationUpdate(
      status: ImagePdfCreationStatus.running,
      totalImageCount: sourcePaths.length,
      completedImageCount: 1,
      currentImage: sourcePaths.length > 1 ? 2 : null,
      outputFiles: const [],
      error: null,
    );
    await Future<void>.delayed(const Duration(milliseconds: 10));
    if (failure case final error?) {
      yield ImagePdfCreationUpdate(
        status: ImagePdfCreationStatus.failed,
        totalImageCount: sourcePaths.length,
        completedImageCount: 0,
        currentImage: 1,
        outputFiles: const [],
        error: error,
      );
      return;
    }
    yield ImagePdfCreationUpdate(
      status: ImagePdfCreationStatus.complete,
      totalImageCount: sourcePaths.length,
      completedImageCount: sourcePaths.length,
      currentImage: null,
      outputFiles: merge
          ? ['$destinationDirectory\\${_stemOf(sourcePaths.first)}.pdf']
          : sourcePaths
                .map((path) => '$destinationDirectory\\${_stemOf(path)}.pdf')
                .toList(),
      error: null,
    );
  }

  @override
  Future<List<SelectedImage>> selectImages() async =>
      selections.isEmpty ? const [] : selections.removeAt(0);

  String _stemOf(String path) {
    final filename = path.split(RegExp(r'[\\/]')).last;
    final dot = filename.lastIndexOf('.');
    return dot <= 0 ? filename : filename.substring(0, dot);
  }
}

class StubPdfToImageWorkflow implements PdfToImageWorkflow {
  @override
  Future<String?> chooseDestinationDirectory() async => null;

  @override
  Stream<PdfImageExportUpdate> exportAllPages({
    required String sourcePath,
    required String destinationDirectory,
    required PdfImageQuality quality,
  }) => const Stream.empty();

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) =>
      throw UnimplementedError();

  @override
  Future<SelectedPdf?> selectAndInspect() async => null;
}

const first = SelectedImage(
  displayName: '01.jpg',
  sourcePath: r'D:\scans\01.jpg',
  sourceDirectory: r'D:\scans',
);
const second = SelectedImage(
  displayName: '02.png',
  sourcePath: r'D:\scans\02.png',
  sourceDirectory: r'D:\scans',
);
const third = SelectedImage(
  displayName: '03.webp',
  sourcePath: r'E:\photos\03.webp',
  sourceDirectory: r'E:\photos',
);

Future<void> pumpImagePanel(
  WidgetTester tester,
  FakeImageToPdfWorkflow workflow,
) async {
  await tester.pumpWidget(
    IlikepdfApp(
      applicationName: 'iLikePDF',
      coreVersion: '0.1.0',
      localOnly: true,
      pdfToImageWorkflow: StubPdfToImageWorkflow(),
      imageToPdfWorkflow: workflow,
    ),
  );
  await tester.tap(find.byKey(const ValueKey('tool-card-images-to-pdf')));
  await tester.pumpAndSettle();
}

void main() {
  test(
    'local workflow prepares picker and dropped paths consistently',
    () async {
      const workflow = LocalImageToPdfWorkflow();
      final selected = await workflow.prepareImagePaths([
        r'D:\scans\photo.JPG',
        r'D:\scans\scan.webp',
      ]);

      expect(selected.map((image) => image.displayName), [
        'photo.JPG',
        'scan.webp',
      ]);
      await expectLater(
        workflow.prepareImagePaths([r'D:\scans\notes.txt']),
        throwsA(
          isA<ImageSelectionException>().having(
            (error) => error.problem.code,
            'problem code',
            ImagePdfProblemCode.unsupportedImageFormat,
          ),
        ),
      );
    },
  );

  testWidgets('empty workspace invites drop or selection', (
    WidgetTester tester,
  ) async {
    final workflow = FakeImageToPdfWorkflow(selections: const []);
    await pumpImagePanel(tester, workflow);

    expect(find.byKey(const ValueKey('empty-image-drop-zone')), findsOneWidget);
    expect(find.text('Drop images here'), findsOneWidget);
    expect(find.text('JPG · JPEG · PNG · WebP'), findsOneWidget);
    expect(find.byType(DropTarget), findsOneWidget);
    final action = tester.widget<FilledButton>(
      find.descendant(
        of: find.byKey(const ValueKey('create-pdf-action')),
        matching: find.byType(FilledButton),
      ),
    );
    expect(action.onPressed, isNull);
  });

  testWidgets('selects multiple images with required defaults', (
    WidgetTester tester,
  ) async {
    final workflow = FakeImageToPdfWorkflow(
      selections: const [
        [first, second],
      ],
    );
    await pumpImagePanel(tester, workflow);

    await tester.tap(find.text('Select images'));
    await tester.pumpAndSettle();

    expect(find.text('2 selected images'), findsOneWidget);
    expect(find.text('01.jpg'), findsOneWidget);
    expect(find.text('02.png'), findsOneWidget);
    expect(find.text(r'D:\scans'), findsOneWidget);
    final pageSize = tester.widget<SegmentedButton<ImagePageSize>>(
      find.byKey(const ValueKey('page-size-control')),
    );
    final orientation = tester.widget<SegmentedButton<ImagePageOrientation>>(
      find.byKey(const ValueKey('orientation-control')),
    );
    final margin = tester.widget<SegmentedButton<ImagePageMargin>>(
      find.byKey(const ValueKey('margin-control')),
    );
    final merge = tester.widget<CheckboxListTile>(
      find.byKey(const ValueKey('merge-images-checkbox')),
    );
    expect(pageSize.selected, {ImagePageSize.a4});
    expect(orientation.selected, {ImagePageOrientation.portrait});
    expect(margin.selected, {ImagePageMargin.none});
    expect(merge.value, isTrue);
  });

  testWidgets('segmented labels stay single-line at wide and narrow widths', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1280, 800);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final workflow = FakeImageToPdfWorkflow(
      selections: const [
        [first],
      ],
    );
    await pumpImagePanel(tester, workflow);
    await tester.tap(find.text('Select images'));
    await tester.pumpAndSettle();

    for (final label in const [
      'Fit',
      'A4',
      'US Letter',
      'Portrait',
      'Landscape',
      'No margin',
      'Small',
      'Big',
    ]) {
      final text = tester.widget<Text>(find.text(label));
      expect(text.maxLines, 1, reason: '$label must stay on one line');
      expect(text.softWrap, isFalse, reason: '$label must not wrap');
    }
    expect(
      tester
          .widget<SegmentedButton<ImagePageSize>>(
            find.byKey(const ValueKey('page-size-control')),
          )
          .showSelectedIcon,
      isFalse,
    );
    expect(tester.takeException(), isNull);

    tester.view.physicalSize = const Size(640, 900);
    await tester.pumpAndSettle();
    expect(find.text('US Letter'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('long destination stays lightweight and does not overflow', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(960, 760);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final workflow = FakeImageToPdfWorkflow(
      selections: const [
        [first],
      ],
      destination: r'C:\Users\banna\Pictures\Screenshots\A very long project folder\Another long nested folder\Final exports',
    );
    await pumpImagePanel(tester, workflow);
    await tester.tap(find.text('Select images'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Choose folder'));
    await tester.tap(find.text('Choose folder'));
    await tester.pumpAndSettle();

    final path = find.byKey(const ValueKey('destination-path'));
    final pathText = tester.widget<Text>(path);
    expect(pathText.maxLines, 2);
    expect(pathText.overflow, TextOverflow.ellipsis);
    expect(
      tester.element(path).findAncestorWidgetOfExactType<DecoratedBox>(),
      isNull,
    );
    expect(
      tester.getRect(path).right,
      lessThanOrEqualTo(
        tester.getRect(find.byKey(const ValueKey('tool-settings-panel'))).right,
      ),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('adds and removes images while preserving custom destination', (
    WidgetTester tester,
  ) async {
    final workflow = FakeImageToPdfWorkflow(
      selections: const [
        [first],
        [second, third],
      ],
    );
    await pumpImagePanel(tester, workflow);
    await tester.tap(find.text('Select images'));
    await tester.pumpAndSettle();

    await tester.ensureVisible(find.text('Choose folder'));
    await tester.tap(find.text('Choose folder'));
    await tester.pumpAndSettle();
    expect(find.text(r'C:\exports'), findsOneWidget);

    await tester.ensureVisible(find.text('Add images'));
    await tester.tap(find.text('Add images'));
    await tester.pumpAndSettle();
    expect(find.text('3 selected images'), findsOneWidget);
    expect(find.text(r'C:\exports'), findsOneWidget);

    await tester.ensureVisible(find.byKey(const ValueKey('remove-image-1')));
    await tester.tap(find.byKey(const ValueKey('remove-image-1')));
    await tester.pumpAndSettle();
    expect(find.text('2 selected images'), findsOneWidget);
    expect(find.text('02.png'), findsNothing);
    expect(find.text(r'C:\exports'), findsOneWidget);
  });

  testWidgets('Fit disables orientation and margin without losing choices', (
    WidgetTester tester,
  ) async {
    final workflow = FakeImageToPdfWorkflow(
      selections: const [
        [first],
      ],
    );
    await pumpImagePanel(tester, workflow);
    await tester.tap(find.text('Select images'));
    await tester.pumpAndSettle();

    await tester.ensureVisible(find.text('Landscape'));
    await tester.tap(find.text('Landscape'));
    await tester.ensureVisible(find.text('Small'));
    await tester.tap(find.text('Small'));
    await tester.ensureVisible(find.text('Fit'));
    await tester.tap(find.text('Fit'));
    await tester.pumpAndSettle();

    final orientation = tester.widget<SegmentedButton<ImagePageOrientation>>(
      find.byKey(const ValueKey('orientation-control')),
    );
    final margin = tester.widget<SegmentedButton<ImagePageMargin>>(
      find.byKey(const ValueKey('margin-control')),
    );
    expect(orientation.onSelectionChanged, isNull);
    expect(margin.onSelectionChanged, isNull);
    expect(orientation.selected, {ImagePageOrientation.landscape});
    expect(margin.selected, {ImagePageMargin.small});
    expect(
      find.text('Fit uses each image’s own page shape with no margin.'),
      findsOneWidget,
    );

    await tester.tap(find.text('US Letter'));
    await tester.pumpAndSettle();
    expect(
      tester
          .widget<SegmentedButton<ImagePageOrientation>>(
            find.byKey(const ValueKey('orientation-control')),
          )
          .selected,
      {ImagePageOrientation.landscape},
    );
  });

  testWidgets('reordering controls the source order passed to Rust', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1280, 800);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final workflow = FakeImageToPdfWorkflow(
      selections: const [
        [first, second, third],
      ],
    );
    await pumpImagePanel(tester, workflow);
    await tester.tap(find.text('Select images'));
    await tester.pumpAndSettle();

    await tester.ensureVisible(find.text('Landscape'));
    await tester.tap(find.text('Landscape'));
    await tester.ensureVisible(find.text('Small'));
    await tester.tap(find.text('Small'));
    await tester.ensureVisible(find.text('Choose folder'));
    await tester.tap(find.text('Choose folder'));
    await tester.pumpAndSettle();

    final handle = find.byKey(const ValueKey('reorder-image-0'));
    await tester.ensureVisible(handle);
    final gesture = await tester.startGesture(tester.getCenter(handle));
    await tester.pump();
    await gesture.moveTo(
      tester.getCenter(find.byKey(const ValueKey('reorder-target-2'))),
    );
    await tester.pump(const Duration(milliseconds: 500));
    await gesture.up();
    await tester.pumpAndSettle();

    expect(
      tester
          .widget<SegmentedButton<ImagePageOrientation>>(
            find.byKey(const ValueKey('orientation-control')),
          )
          .selected,
      {ImagePageOrientation.landscape},
    );
    expect(
      tester
          .widget<SegmentedButton<ImagePageMargin>>(
            find.byKey(const ValueKey('margin-control')),
          )
          .selected,
      {ImagePageMargin.small},
    );
    expect(find.text(r'C:\exports'), findsOneWidget);

    await tester.ensureVisible(find.text('Create PDF'));
    await tester.tap(find.text('Create PDF'));
    await tester.pumpAndSettle();
    expect(workflow.createdSourcePaths, [
      second.sourcePath,
      third.sourcePath,
      first.sourcePath,
    ]);
    expect(find.text('PDF creation complete: 1 file created.'), findsOneWidget);
    _expectStatusBottomSpacing(tester, const ValueKey('feedback-success'));
  });

  testWidgets(
    'native drop uses workflow ingestion and adds to existing cards',
    (WidgetTester tester) async {
      final workflow = FakeImageToPdfWorkflow(
        selections: const [
          [first],
        ],
      );
      await pumpImagePanel(tester, workflow);

      await _dropFiles(tester, [second.sourcePath]);
      expect(workflow.preparedDroppedPaths, [second.sourcePath]);
      expect(find.text('02.png'), findsOneWidget);
      expect(find.text('1 selected image'), findsOneWidget);
      expect(find.text(r'D:\scans'), findsOneWidget);

      await tester.ensureVisible(find.text('Add images'));
      await tester.tap(find.text('Add images'));
      await tester.pumpAndSettle();
      await _dropFiles(tester, [third.sourcePath]);

      expect(find.text('3 selected images'), findsOneWidget);
      expect(find.text('01.jpg'), findsOneWidget);
      expect(find.text('02.png'), findsOneWidget);
      expect(find.text('03.webp'), findsOneWidget);
    },
  );

  testWidgets('unsupported native drop shows the normal format error', (
    WidgetTester tester,
  ) async {
    final workflow = FakeImageToPdfWorkflow(selections: const []);
    await pumpImagePanel(tester, workflow);

    await _dropFiles(tester, [r'D:\scans\notes.txt']);

    expect(
      find.text('Only JPG, JPEG, PNG, and WebP images are supported.'),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('empty-image-drop-zone')), findsOneWidget);
  });

  testWidgets('structured failure has bottom spacing in the narrow layout', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(760, 900);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final workflow = FakeImageToPdfWorkflow(
      selections: const [
        [first, second],
      ],
      failure: const ImagePdfProblem(
        code: ImagePdfProblemCode.outputWriteFailed,
        message: 'The output PDF could not be written',
      ),
    );
    await pumpImagePanel(tester, workflow);
    await tester.tap(find.text('Select images'));
    await tester.pumpAndSettle();

    await tester.ensureVisible(
      find.byKey(const ValueKey('merge-images-checkbox')),
    );
    await tester.tap(find.text('Merge all images in one PDF file'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Create PDFs'));
    await tester.tap(find.text('Create PDFs'));
    await tester.pumpAndSettle();

    expect(workflow.createdMerge, isFalse);
    expect(
      find.text(
        'PDF creation failed at image 1. The output PDF could not be written No PDFs were created.',
      ),
      findsOneWidget,
    );
    _expectStatusBottomSpacing(tester, const ValueKey('feedback-error'));
  });

  testWidgets('running status remains separate from the primary action', (
    WidgetTester tester,
  ) async {
    final workflow = FakeImageToPdfWorkflow(
      selections: const [
        [first, second],
      ],
    );
    await pumpImagePanel(tester, workflow);
    await tester.tap(find.text('Select images'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Create PDF'));
    await tester.tap(find.text('Create PDF'));
    await tester.pump();

    expect(find.byKey(const ValueKey('feedback-running')), findsOneWidget);
    _expectStatusBottomSpacing(tester, const ValueKey('feedback-running'));
    expect(tester.takeException(), isNull);
    await tester.pumpAndSettle();
  });
}

void _expectStatusBottomSpacing(WidgetTester tester, Key feedbackKey) {
  final status = tester.getRect(
    find.byKey(const ValueKey('tool-status-region')),
  );
  final feedback = tester.getRect(find.byKey(feedbackKey));
  final action = tester.getRect(
    find.byKey(const ValueKey('create-pdf-action')),
  );
  expect(status.bottom - feedback.bottom, greaterThanOrEqualTo(16));
  expect(action.top, greaterThan(status.bottom));
}

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
