import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/editor/editor_panel.dart';
import 'package:ilikepdf/src/app/editor/editor_viewport.dart';
import 'package:ilikepdf/src/app/editor/editor_workflow.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

class _Workflow implements EditorWorkflow {
  final pending = <Completer<EditorSession>>[];
  @override
  Future<EditorSource?> selectPdf() async =>
      const EditorSource('fixture.pdf', 'Fixture PDF');
  @override
  Future<EditorSession> open(EditorSource source) {
    final request = Completer<EditorSession>();
    pending.add(request);
    return request.future;
  }
}

class _Binding implements EditorLayoutBinding {
  @override
  Uint32List visiblePages({
    required double top,
    required double height,
    required double overscan,
  }) => Uint32List.fromList([0]);
  @override
  EditorRasterSize renderSize({
    required int pageIndex,
    required double density,
  }) => const EditorRasterSize(width: 100, height: 200);
  @override
  EditorPoint anchoredScroll({
    required EditorLayoutBinding previous,
    required EditorPoint scroll,
    required EditorPoint viewport,
  }) => scroll;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Transform implements EditorPageTransform {
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Session implements EditorSession {
  _Session(this.id);
  final int id;
  var layoutCalls = 0;
  var renderCalls = 0;
  @override
  BigInt identity() => BigInt.from(id);
  @override
  int pageCount() => 1;
  @override
  EditorDocumentLayout layout({
    required double workspaceWidth,
    required double scale,
    required bool fitWidth,
  }) {
    layoutCalls++;
    return EditorDocumentLayout(
      binding: _Binding(),
      width: workspaceWidth,
      height: 248,
      scale: 1,
      pages: [
        EditorPageLayout(
          pageIndex: 0,
          rect: const EditorRect(left: 24, top: 24, width: 100, height: 200),
          scale: 1,
          geometry: const EditorPageGeometry(
            visibleBox: EditorPageBox(left: 0, bottom: 0, right: 100, top: 200),
            rotation: EditorPageRotation.none,
          ),
          transform: _Transform(),
        ),
      ],
    );
  }

  @override
  Future<EditorRaster> render({
    required int pageIndex,
    required EditorRasterSize size,
  }) async {
    renderCalls++;
    throw const ApplicationError(
      code: ApplicationErrorCode.renderingFailed,
      message: 'This page could not be rendered.',
    );
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  testWidgets(
    'closing an opening session rejects late results and the next session opens normally',
    (tester) async {
      final workflow = _Workflow();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: EditorPanel(workflow: workflow)),
        ),
      );
      expect(find.byType(EditorViewport), findsNothing);
      await tester.tap(find.byKey(const ValueKey('editor-open')));
      await tester.pump();
      expect(find.byKey(const ValueKey('editor-opening')), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('editor-close')));
      await tester.pump();
      workflow.pending[0].complete(_Session(1));
      await tester.pumpAndSettle();
      expect(find.byType(EditorViewport), findsNothing);
      await tester.tap(find.byKey(const ValueKey('editor-open')));
      await tester.pump();
      final current = _Session(2);
      workflow.pending[1].complete(current);
      await tester.pumpAndSettle();
      expect(find.text('Fixture PDF · 1 pages'), findsOneWidget);
      expect(find.text('This page could not be rendered.'), findsOneWidget);
      expect(current.renderCalls, 1);
      await tester.pumpAndSettle();
      expect(current.renderCalls, 1);
      expect(current.layoutCalls, 1);
      await tester.ensureVisible(find.text('Retry page'));
      await tester.tap(find.text('Retry page'));
      await tester.pumpAndSettle();
      expect(current.renderCalls, 2);
    },
  );

  testWidgets(
    'failed replacement preserves the current viewport and reports a safe error',
    (tester) async {
      final workflow = _Workflow();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: EditorPanel(workflow: workflow)),
        ),
      );
      await tester.tap(find.byKey(const ValueKey('editor-open')));
      await tester.pump();
      workflow.pending[0].complete(_Session(1));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('editor-open')));
      await tester.pump();
      workflow.pending[1].completeError(
        const ApplicationError(
          code: ApplicationErrorCode.invalidPdf,
          message: 'This PDF is invalid.',
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('This PDF is invalid.'), findsOneWidget);
      expect(find.byType(EditorViewport), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('editor-close')));
      await tester.pumpAndSettle();
      expect(find.byType(EditorViewport), findsNothing);
      expect(find.byKey(const ValueKey('editor-open-error')), findsNothing);
    },
  );
}
