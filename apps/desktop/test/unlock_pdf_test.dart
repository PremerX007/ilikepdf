import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/shared/pdf_encryption_state.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';
import 'package:ilikepdf/src/app/unlock_pdf/unlock_pdf_panel.dart';
import 'package:ilikepdf/src/app/unlock_pdf/unlock_pdf_workflow.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

class FakeUnlockPdfWorkflow implements UnlockPdfWorkflow {
  FakeUnlockPdfWorkflow({required this.previewPath});

  final String previewPath;
  final List<SelectedUnlockPdf?> selections = [];
  List<UnlockPdfUpdate> updates = const [];
  String? submittedPassword;
  int submissions = 0;

  @override
  Future<String?> chooseDestinationDirectory() async => r'D:\exports';

  @override
  Future<SelectedUnlockPdf> preparePdfPaths(List<String> sourcePaths) async =>
      selections.removeAt(0)!;

  @override
  Future<RenderedUnlockPdfPage> renderFirstPage(String sourcePath) async =>
      RenderedUnlockPdfPage(outputPath: previewPath);

  @override
  Future<SelectedUnlockPdf?> selectPdf() async => selections.removeAt(0);

  @override
  Stream<UnlockPdfUpdate> unlock({
    required String sourcePath,
    required String destinationDirectory,
    required String outputName,
    required String? password,
  }) async* {
    submissions++;
    submittedPassword = password;
    for (final update in updates) {
      yield update;
    }
  }
}

SelectedUnlockPdf source(PdfEncryptionState state) => SelectedUnlockPdf(
  displayName: 'report.pdf',
  sourcePath: r'C:\docs\report.pdf',
  sourceDirectory: r'C:\docs',
  pageCount: state == PdfEncryptionState.encryptedPasswordRequired ? null : 2,
  sizeBytes: 100,
  encryptionState: state,
  hasWarnings: false,
  defaultOutputName: 'report-unlocked.pdf',
);

void main() {
  late Directory previewDirectory;
  late String previewPath;

  setUpAll(() async {
    previewDirectory = await Directory.systemTemp.createTemp('unlock-widget-');
    final file = File('${previewDirectory.path}\\preview.png');
    await file.writeAsBytes(
      base64Decode(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
      ),
    );
    previewPath = file.path;
  });
  tearDownAll(() => previewDirectory.delete(recursive: true));

  testWidgets(
    'unencrypted state shows already unlocked with no password and disabled action',
    (tester) async {
      final workflow = FakeUnlockPdfWorkflow(previewPath: previewPath)
        ..selections.add(source(PdfEncryptionState.unencrypted));
      await _pump(tester, workflow);
      await tester.tap(find.text('Select PDF'));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const ValueKey('unlock-already-unlocked')),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('unlock-password')), findsNothing);
      expect(_action(tester).onPressed, isNull);
    },
  );

  testWidgets('encrypted no-password-required state unlocks directly', (
    tester,
  ) async {
    final workflow = FakeUnlockPdfWorkflow(previewPath: previewPath)
      ..selections.add(source(PdfEncryptionState.encryptedNoPasswordRequired))
      ..updates = const [
        UnlockPdfUpdate(
          status: UnlockPdfUpdateStatus.complete,
          stage: UnlockPdfProgressStage.completed,
          pageCount: 2,
          outputPath: r'C:\docs\report-unlocked.pdf',
          hasWarnings: false,
          error: null,
        ),
      ];
    await _pump(tester, workflow);
    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey('unlock-no-password-required')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('unlock-password')), findsNothing);
    expect(_action(tester).onPressed, isNotNull);
    await tester.ensureVisible(find.byKey(const ValueKey('unlock-pdf-action')));
    await tester.tap(find.byKey(const ValueKey('unlock-pdf-action')));
    await tester.pumpAndSettle();
    expect(workflow.submittedPassword, isNull);
    expect(find.byKey(const ValueKey('unlock-success')), findsOneWidget);
  });

  testWidgets(
    'password field appears only on demand and wrong password remains for retry',
    (tester) async {
      final workflow = FakeUnlockPdfWorkflow(previewPath: previewPath)
        ..selections.add(source(PdfEncryptionState.encryptedPasswordRequired))
        ..updates = const [
          UnlockPdfUpdate(
            status: UnlockPdfUpdateStatus.failed,
            stage: UnlockPdfProgressStage.preparing,
            pageCount: 0,
            outputPath: null,
            hasWarnings: false,
            error: UnlockPdfProblem(
              code: ApplicationErrorCode.incorrectPassword,
              message: 'The password is incorrect. Try again',
            ),
          ),
        ];
      await _pump(tester, workflow);
      await tester.tap(find.text('Select PDF'));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const ValueKey('unlock-password-required')),
        findsOneWidget,
      );
      final field = find.byKey(const ValueKey('unlock-password'));
      expect(field, findsOneWidget);
      expect(tester.widget<TextField>(field).obscureText, isTrue);
      expect(_action(tester).onPressed, isNull);
      await tester.enterText(field, 'retry-locally');
      await tester.pump();
      expect(_action(tester).onPressed, isNotNull);
      await tester.ensureVisible(
        find.byKey(const ValueKey('unlock-pdf-action')),
      );
      await tester.tap(find.byKey(const ValueKey('unlock-pdf-action')));
      await tester.pumpAndSettle();

      expect(find.byKey(const ValueKey('unlock-failure')), findsOneWidget);
      expect(find.text('The password is incorrect. Try again'), findsOneWidget);
      expect(tester.widget<TextField>(field).controller!.text, 'retry-locally');
      expect(_visibleText('retry-locally'), findsNothing);
    },
  );

  testWidgets(
    'successful password unlock clears the local password controller',
    (tester) async {
      final workflow = FakeUnlockPdfWorkflow(previewPath: previewPath)
        ..selections.add(source(PdfEncryptionState.encryptedPasswordRequired))
        ..updates = const [
          UnlockPdfUpdate(
            status: UnlockPdfUpdateStatus.running,
            stage: UnlockPdfProgressStage.unlocking,
            pageCount: 2,
            outputPath: null,
            hasWarnings: false,
            error: null,
          ),
          UnlockPdfUpdate(
            status: UnlockPdfUpdateStatus.complete,
            stage: UnlockPdfProgressStage.completed,
            pageCount: 2,
            outputPath: r'C:\docs\report-unlocked.pdf',
            hasWarnings: false,
            error: null,
          ),
        ];
      await _pump(tester, workflow);
      await tester.tap(find.text('Select PDF'));
      await tester.pumpAndSettle();
      final field = find.byKey(const ValueKey('unlock-password'));
      await tester.enterText(field, 'correct-value');
      await tester.pump();
      await tester.ensureVisible(
        find.byKey(const ValueKey('unlock-pdf-action')),
      );
      await tester.tap(find.byKey(const ValueKey('unlock-pdf-action')));
      await tester.pumpAndSettle();

      expect(workflow.submissions, 1);
      expect(workflow.submittedPassword, 'correct-value');
      expect(tester.widget<TextField>(field).controller!.text, isEmpty);
      expect(find.byKey(const ValueKey('unlock-success')), findsOneWidget);
    },
  );
}

PrimaryToolAction _action(WidgetTester tester) => tester
    .widget<PrimaryToolAction>(find.byKey(const ValueKey('unlock-pdf-action')));

Finder _visibleText(String value) =>
    find.byWidgetPredicate((widget) => widget is Text && widget.data == value);

Future<void> _pump(WidgetTester tester, UnlockPdfWorkflow workflow) =>
    tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: UnlockPdfPanel(workflow: workflow)),
      ),
    );
