import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/protect_pdf/protect_pdf_panel.dart';
import 'package:ilikepdf/src/app/protect_pdf/protect_pdf_workflow.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/pdf_encryption_state.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

class FakeProtectPdfWorkflow implements ProtectPdfWorkflow {
  FakeProtectPdfWorkflow({required this.previewPath});

  final String previewPath;
  final List<SelectedProtectPdf?> selections = [];
  List<ProtectPdfUpdate> updates = const [];
  int submissions = 0;
  String? submittedPassword;
  String? submittedConfirmation;

  @override
  Future<String?> chooseDestinationDirectory() async => r'D:\exports';

  @override
  Future<SelectedProtectPdf> preparePdfPaths(List<String> sourcePaths) async {
    if (sourcePaths.length != 1) {
      throw const ProtectPdfSelectionException(
        ProtectPdfProblem(
          code: ApplicationErrorCode.invalidRequest,
          message: 'Protect PDF supports one PDF at a time.',
        ),
      );
    }
    return selections.removeAt(0)!;
  }

  @override
  Stream<ProtectPdfUpdate> protect({
    required String sourcePath,
    required String destinationDirectory,
    required String outputName,
    required String password,
    required String confirmation,
  }) async* {
    submissions++;
    submittedPassword = password;
    submittedConfirmation = confirmation;
    for (final update in updates) {
      yield update;
    }
  }

  @override
  Future<RenderedProtectPdfPage> renderFirstPage(String sourcePath) async =>
      RenderedProtectPdfPage(outputPath: previewPath);

  @override
  Future<SelectedProtectPdf?> selectPdf() async => selections.removeAt(0);
}

const unencrypted = SelectedProtectPdf(
  displayName: 'report.pdf',
  sourcePath: r'C:\docs\report.pdf',
  sourceDirectory: r'C:\docs',
  pageCount: 2,
  sizeBytes: 100,
  encryptionState: PdfEncryptionState.unencrypted,
  hasWarnings: false,
  defaultOutputName: 'report-protected.pdf',
);

const encrypted = SelectedProtectPdf(
  displayName: 'encrypted.pdf',
  sourcePath: r'C:\docs\encrypted.pdf',
  sourceDirectory: r'C:\docs',
  pageCount: null,
  sizeBytes: 100,
  encryptionState: PdfEncryptionState.encryptedPasswordRequired,
  hasWarnings: false,
  defaultOutputName: 'encrypted-protected.pdf',
);

void main() {
  late Directory previewDirectory;
  late String previewPath;

  setUpAll(() async {
    previewDirectory = await Directory.systemTemp.createTemp('protect-widget-');
    final file = File('${previewDirectory.path}\\preview.png');
    await file.writeAsBytes(
      base64Decode(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
      ),
    );
    previewPath = file.path;
  });

  tearDownAll(() => previewDirectory.delete(recursive: true));

  testWidgets('single source exposes obscured matching password controls', (
    tester,
  ) async {
    final workflow = FakeProtectPdfWorkflow(previewPath: previewPath)
      ..selections.add(unencrypted);
    await _pump(tester, workflow);

    expect(
      find.byKey(const ValueKey('empty-protect-pdf-drop-zone')),
      findsOneWidget,
    );
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();

    expect(find.text('report.pdf'), findsOneWidget);
    expect(find.text('report-protected.pdf'), findsOneWidget);
    expect(
      tester
          .widget<TextField>(find.byKey(const ValueKey('protect-password')))
          .obscureText,
      isTrue,
    );
    expect(
      tester
          .widget<TextField>(
            find.byKey(const ValueKey('protect-password-confirmation')),
          )
          .obscureText,
      isTrue,
    );
    expect(_action(tester).onPressed, isNull);

    await tester.enterText(
      find.byKey(const ValueKey('protect-password')),
      'รหัสผ่าน',
    );
    await tester.enterText(
      find.byKey(const ValueKey('protect-password-confirmation')),
      'different',
    );
    await tester.pump();
    expect(find.text('Passwords do not match.'), findsOneWidget);
    expect(_action(tester).onPressed, isNull);
    await tester.enterText(
      find.byKey(const ValueKey('protect-password-confirmation')),
      'รหัสผ่าน',
    );
    await tester.pump();
    expect(_action(tester).onPressed, isNotNull);
  });

  testWidgets(
    'multiple drop and encrypted replacement preserve current valid session',
    (tester) async {
      final workflow = FakeProtectPdfWorkflow(previewPath: previewPath)
        ..selections.add(unencrypted);
      await _pump(tester, workflow);
      await tester.tap(find.text('Select PDF'));
      await tester.pumpAndSettle();

      final dropZone = tester.widget<FileDropZone>(find.byType(FileDropZone));
      await dropZone.onDroppedPaths(['one.pdf', 'two.pdf']);
      await tester.pumpAndSettle();
      expect(find.text('report.pdf'), findsOneWidget);
      expect(find.textContaining('one PDF at a time'), findsOneWidget);

      workflow.selections.add(encrypted);
      await tester.tap(find.byKey(const ValueKey('replace-protect-pdf')));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey('protect-already-encrypted')),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('protect-password')), findsNothing);
      expect(_action(tester).onPressed, isNull);
    },
  );

  testWidgets('success uses one password submission then clears both fields', (
    tester,
  ) async {
    final workflow = FakeProtectPdfWorkflow(previewPath: previewPath)
      ..selections.add(unencrypted)
      ..updates = const [
        ProtectPdfUpdate(
          status: ProtectPdfUpdateStatus.running,
          stage: ProtectPdfProgressStage.protecting,
          pageCount: 2,
          outputPath: null,
          hasWarnings: false,
          error: null,
        ),
        ProtectPdfUpdate(
          status: ProtectPdfUpdateStatus.complete,
          stage: ProtectPdfProgressStage.completed,
          pageCount: 2,
          outputPath: r'C:\docs\report-protected.pdf',
          hasWarnings: false,
          error: null,
        ),
      ];
    await _pump(tester, workflow);
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('protect-password')),
      'private-value',
    );
    await tester.enterText(
      find.byKey(const ValueKey('protect-password-confirmation')),
      'private-value',
    );
    await tester.pump();
    await tester.ensureVisible(
      find.byKey(const ValueKey('protect-pdf-action')),
    );
    await tester.tap(find.byKey(const ValueKey('protect-pdf-action')));
    await tester.pumpAndSettle();

    expect(workflow.submissions, 1);
    expect(workflow.submittedPassword, 'private-value');
    expect(find.byKey(const ValueKey('protect-success')), findsOneWidget);
    expect(_visibleText('private-value'), findsNothing);
    expect(
      tester
          .widget<TextField>(find.byKey(const ValueKey('protect-password')))
          .controller!
          .text,
      isEmpty,
    );
  });

  testWidgets('structured failure never renders submitted password', (
    tester,
  ) async {
    final workflow = FakeProtectPdfWorkflow(previewPath: previewPath)
      ..selections.add(unencrypted)
      ..updates = const [
        ProtectPdfUpdate(
          status: ProtectPdfUpdateStatus.failed,
          stage: ProtectPdfProgressStage.protecting,
          pageCount: 0,
          outputPath: null,
          hasWarnings: false,
          error: ProtectPdfProblem(
            code: ApplicationErrorCode.structuralPdfOperationFailed,
            message: 'The PDF could not be protected.',
          ),
        ),
      ];
    await _pump(tester, workflow);
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('protect-password')),
      'never-visible',
    );
    await tester.enterText(
      find.byKey(const ValueKey('protect-password-confirmation')),
      'never-visible',
    );
    await tester.pump();
    await tester.ensureVisible(
      find.byKey(const ValueKey('protect-pdf-action')),
    );
    await tester.tap(find.byKey(const ValueKey('protect-pdf-action')));
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('protect-failure')), findsOneWidget);
    expect(_visibleText('never-visible'), findsNothing);
  });
}

PrimaryToolAction _action(WidgetTester tester) =>
    tester.widget<PrimaryToolAction>(
      find.byKey(const ValueKey('protect-pdf-action')),
    );

Finder _visibleText(String value) =>
    find.byWidgetPredicate((widget) => widget is Text && widget.data == value);

Future<void> _pump(WidgetTester tester, ProtectPdfWorkflow workflow) =>
    tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: ProtectPdfPanel(workflow: workflow)),
      ),
    );
