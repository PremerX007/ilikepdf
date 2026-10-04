import 'dart:async';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/editor/editor_object_overlay.dart';
import 'package:ilikepdf/src/app/editor/editor_panel.dart';
import 'package:ilikepdf/src/app/editor/editor_render_cache.dart';
import 'package:ilikepdf/src/app/editor/editor_viewport.dart';
import 'package:ilikepdf/src/app/editor/editor_workflow.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

File _fixture(String name) =>
    File('../../crates/ilikepdf_pdf/tests/fixtures/$name').absolute;
Future<EditorSession> _open(
  WidgetTester tester, {
  required String sourcePath,
}) async =>
    (await tester.runAsync(() => openEditorSession(sourcePath: sourcePath)))!;
Future<void> _settle(WidgetTester tester, EditorRenderCache cache) async {
  await tester.pump();
  await tester.runAsync(() => cache.idle);
  await tester.pumpAndSettle();
}

Future<void> _mouseDrag(
  WidgetTester tester,
  Offset start,
  Offset delta, {
  int frames = 1,
}) async {
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await mouse.addPointer(location: start);
  await mouse.down(start);
  for (var i = 1; i <= frames; i++) {
    await mouse.moveTo(start + delta * (i / frames));
    await tester.pump();
  }
  await mouse.up();
  await mouse.removePointer();
  await tester.pump();
}

Widget _app(
  EditorSession session,
  EditorEdits edits,
  EditorRenderCache cache, {
  Widget? input,
}) => MaterialApp(
  home: Scaffold(
    body: Column(
      children: [
        ?input,
        Expanded(
          child: EditorViewport(session: session, edits: edits, cache: cache),
        ),
      ],
    ),
  ),
);
Finder _object(BigInt id) => find.byKey(ValueKey('editor-object-$id'));
Future<void> _shortcut(
  WidgetTester tester,
  LogicalKeyboardKey key, {
  bool shift = false,
}) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  if (shift) await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(key);
  if (shift) await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

class _DelayedSession implements EditorSession {
  _DelayedSession(this.inner);
  final EditorSession inner;
  final pending = <Completer<EditorRaster>>[];
  @override
  BigInt identity() => inner.identity();
  @override
  EditorEdits createEdits() => inner.createEdits();
  @override
  EditorDocumentLayout layout({
    required double workspaceWidth,
    required double scale,
    required bool fitWidth,
  }) => inner.layout(
    workspaceWidth: workspaceWidth,
    scale: scale,
    fitWidth: fitWidth,
  );
  @override
  Future<EditorRaster> render({
    required int pageIndex,
    required EditorRasterSize size,
  }) {
    final c = Completer<EditorRaster>();
    pending.add(c);
    return c.future;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
  void finish() {
    for (final c in pending) {
      if (!c.isCompleted) {
        c.completeError(
          const ApplicationError(
            code: ApplicationErrorCode.renderingFailed,
            message: 'Controlled render completion',
          ),
        );
      }
    }
  }
}

class _ReplacementWorkflow implements EditorWorkflow {
  final requests = <Completer<EditorSession>>[];
  @override
  Future<EditorSource?> selectPdf() async =>
      const EditorSource('fixture.pdf', 'Fixture PDF');
  @override
  Future<EditorSession> open(EditorSource source) {
    final c = Completer<EditorSession>();
    requests.add(c);
    return c.future;
  }
}

void editorObjectScenarios() {
  testWidgets(
    'failed replacement preserves objects selection and both history branches; successful replacement clears them',
    (tester) async {
      final source = await _open(
        tester,
        sourcePath: _fixture('two_page.pdf').path,
      );
      final workflow = _ReplacementWorkflow();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: EditorPanel(workflow: workflow)),
        ),
      );
      await tester.tap(find.byKey(const ValueKey('editor-open')));
      await tester.pump();
      workflow.requests[0].complete(source);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('editor-add-object')));
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('editor-add-object')));
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('editor-undo')));
      await tester.pump();
      final old = tester
          .widget<EditorViewport>(find.byType(EditorViewport))
          .edits!;
      await tester.tapAt(
        tester.getCenter(_object(old.snapshot().objects.single.id)),
      );
      await tester.pump();
      final before = old.snapshot();
      await tester.tap(find.byKey(const ValueKey('editor-open')));
      await tester.pump();
      workflow.requests[1].completeError(
        const ApplicationError(
          code: ApplicationErrorCode.invalidPdf,
          message: 'This PDF is invalid.',
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('This PDF is invalid.'), findsOneWidget);
      final retained = tester
          .widget<EditorViewport>(find.byType(EditorViewport))
          .edits!;
      expect(retained.snapshot().objects, before.objects);
      expect(retained.snapshot().selected, before.selected);
      expect(retained.snapshot().undoCount, before.undoCount);
      expect(retained.snapshot().redoCount, before.redoCount);
      await tester.tap(find.byKey(const ValueKey('editor-open')));
      await tester.pump();
      workflow.requests[2].complete(
        await _open(tester, sourcePath: _fixture('two_page.pdf').path),
      );
      await tester.pumpAndSettle();
      final fresh = tester
          .widget<EditorViewport>(find.byType(EditorViewport))
          .edits!
          .snapshot();
      expect(fresh.objects, isEmpty);
      expect(fresh.selected, isNull);
      expect(fresh.undoCount, 0);
      expect(fresh.redoCount, 0);
      await tester.tap(find.byKey(const ValueKey('editor-close')));
      await tester.pumpAndSettle();
      expect(find.byType(EditorViewport), findsNothing);
      await tester.pumpWidget(const SizedBox());
      await tester.pump();
    },
  );

  testWidgets(
    '100 to 200 to 400 to 100 native projection and raster replacement leave canonical objects unchanged',
    (tester) async {
      final source = _fixture('editor_geometry.pdf');
      final bytes = await tester.runAsync(source.readAsBytes);
      final session = await _open(tester, sourcePath: source.path);
      final edits = session.createEdits();
      for (var page = 0; page < 6; page++) {
        edits.addPrototype(pageIndex: page);
      }
      final before = edits.snapshot();
      final initial = session.layout(
        workspaceWidth: 900,
        scale: 1,
        fitWidth: false,
      );
      for (final scale in [1.0, 2.0, 4.0, 1.0]) {
        final layout = session.layout(
          workspaceWidth: 900,
          scale: scale,
          fitWidth: false,
        );
        for (var index = 0; index < 6; index++) {
          final object = before.objects[index];
          final b = object.rectangle;
          final page = layout.pages[index];
          final rect = edits
              .projectPage(layout: layout.binding, pageIndex: index)
              .single
              .rect;
          for (final p in [
            EditorPoint(x: b.left, y: b.bottom),
            EditorPoint(x: b.right, y: b.top),
          ]) {
            final projected = page.transform.pdfToDocument(point: p);
            expect(
              projected.x,
              anyOf(
                closeTo(rect.left, 1e-7),
                closeTo(rect.left + rect.width, 1e-7),
              ),
            );
            expect(
              projected.y,
              anyOf(
                closeTo(rect.top, 1e-7),
                closeTo(rect.top + rect.height, 1e-7),
              ),
            );
          }
          final atOne = edits
              .projectPage(layout: initial.binding, pageIndex: index)
              .single
              .rect;
          expect(
            rect.left - page.rect.left,
            closeTo(
              (atOne.left - initial.pages[index].rect.left) * scale,
              1e-7,
            ),
          );
          expect(
            rect.top - page.rect.top,
            closeTo((atOne.top - initial.pages[index].rect.top) * scale, 1e-7),
          );
          if (index == 2) {
            final size = layout.binding.renderSize(
              pageIndex: index,
              density: 2,
            );
            final raster = await tester.runAsync(
              () => session.render(pageIndex: index, size: size),
            );
            expect(raster!.width, size.width);
            expect(
              edits
                  .projectPage(layout: layout.binding, pageIndex: index)
                  .single
                  .rect,
              rect,
            );
          }
        }
        expect(edits.snapshot().objects, before.objects);
        expect(edits.snapshot().undoCount, before.undoCount);
      }
      final stale = edits.beginGesture(
        layout: initial.binding,
        point: initial.pages[0].transform.pdfToDocument(
          point: EditorPoint(
            x:
                (before.objects[0].rectangle.left +
                    before.objects[0].rectangle.right) /
                2,
            y:
                (before.objects[0].rectangle.bottom +
                    before.objects[0].rectangle.top) /
                2,
          ),
        ),
      )!;
      edits.addPrototype(pageIndex: 0);
      expect(
        () => edits.finishGesture(
          gesture: stale,
          point: const EditorPoint(x: 100, y: 100),
        ),
        throwsA(isA<ApplicationError>()),
      );
      expect(await tester.runAsync(source.readAsBytes), bytes);
    },
  );
  testWidgets(
    'prototype selection, z order, gesture transactions, delete and keyboard history',
    (tester) async {
      final source = _fixture('editor_geometry.pdf');
      final bytes = await tester.runAsync(source.readAsBytes);
      final session = await _open(tester, sourcePath: source.path);
      final edits = session.createEdits();
      final cache = EditorRenderCache();
      await tester.pumpWidget(_app(session, edits, cache));
      await _settle(tester, cache);
      await tester.tap(find.byKey(const ValueKey('editor-reset-zoom')));
      await _settle(tester, cache);
      await tester.tap(find.byKey(const ValueKey('editor-add-object')));
      await tester.pump();
      final a = edits.snapshot().objects.single.id;
      expect(_object(a), findsOneWidget);
      expect(edits.snapshot().selected, a);
      final original = edits.snapshot().objects.single.rectangle;
      final page = find.byKey(
        ValueKey('editor-page-${edits.snapshot().objects.single.pageIndex}'),
      );
      await tester.tapAt(tester.getTopLeft(page) + const Offset(15, 15));
      await tester.pump();
      expect(edits.snapshot().selected, isNull);
      await tester.tapAt(tester.getCenter(_object(a)));
      await tester.pump();
      expect(edits.snapshot().selected, a);
      await tester.tap(find.byKey(const ValueKey('editor-add-object')));
      await tester.pump();
      final b = edits.snapshot().objects.last.id;
      await tester.tapAt(tester.getCenter(_object(a)));
      await tester.pump();
      expect(edits.snapshot().selected, b);
      final undoBefore = edits.snapshot().undoCount;
      await _mouseDrag(
        tester,
        tester.getCenter(_object(b)),
        const Offset(20, 15),
        frames: 80,
      );
      var snapshot = edits.snapshot();
      expect(snapshot.undoCount, undoBefore + 1);
      final moved = snapshot.objects.last.rectangle;
      expect(moved.left, closeTo(original.left + 20, 1e-7));
      expect(moved.bottom, closeTo(original.bottom - 15, 1e-7));
      final handle = find.byKey(ValueKey('editor-handle-$b-2'));
      await _mouseDrag(
        tester,
        tester.getCenter(handle),
        const Offset(12, -10),
        frames: 80,
      );
      snapshot = edits.snapshot();
      expect(snapshot.undoCount, undoBefore + 2);
      final resized = snapshot.objects.last.rectangle;
      expect(resized.left, closeTo(moved.left, 1e-7));
      expect(resized.bottom, closeTo(moved.bottom, 1e-7));
      expect(resized.right, closeTo(moved.right + 12, 1e-7));
      expect(resized.top, closeTo(moved.top + 10, 1e-7));
      await tester.sendKeyEvent(LogicalKeyboardKey.delete);
      await tester.pump();
      expect(_object(b), findsNothing);
      await _shortcut(tester, LogicalKeyboardKey.keyZ);
      expect(edits.snapshot().objects.last.id, b);
      expect(_object(b), findsOneWidget);
      await _shortcut(tester, LogicalKeyboardKey.keyY);
      expect(_object(b), findsNothing);
      await _shortcut(tester, LogicalKeyboardKey.keyZ);
      await _shortcut(tester, LogicalKeyboardKey.keyZ);
      expect(edits.snapshot().objects.last.rectangle, moved);
      await _shortcut(tester, LogicalKeyboardKey.keyZ);
      expect(edits.snapshot().objects.last.rectangle, original);
      await _shortcut(tester, LogicalKeyboardKey.keyZ);
      expect(edits.snapshot().objects.single.id, a);
      await _shortcut(tester, LogicalKeyboardKey.keyZ);
      expect(edits.snapshot().objects, isEmpty);
      await _shortcut(tester, LogicalKeyboardKey.keyZ, shift: true);
      expect(edits.snapshot().objects.single.id, a);
      await tester.tap(find.byKey(const ValueKey('editor-add-object')));
      await tester.pump();
      expect(edits.snapshot().redoCount, 0);
      expect(await tester.runAsync(source.readAsBytes), bytes);
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets(
    'overlay remains projected through zoom scroll raster arrival and every rotation',
    (tester) async {
      final session = await _open(
        tester,
        sourcePath: _fixture('editor_geometry.pdf').path,
      );
      final edits = session.createEdits();
      final cache = EditorRenderCache();
      for (final index in [2, 3, 4, 5]) {
        edits.addPrototype(pageIndex: index);
      }
      await tester.pumpWidget(_app(session, edits, cache));
      await _settle(tester, cache);
      final objects = edits.snapshot().objects;
      final history = edits.snapshot().undoCount;
      final workspace = find.byKey(const ValueKey('editor-workspace'));
      final vertical = tester
          .widget<SingleChildScrollView>(
            find.byKey(const ValueKey('editor-vertical-scroll')),
          )
          .controller!;
      for (final index in [2, 3, 4, 5]) {
        await tester.tap(find.byKey(const ValueKey('editor-reset-zoom')));
        await _settle(tester, cache);
        final layout = session.layout(
          workspaceWidth: tester.getSize(workspace).width,
          scale: 1,
          fitWidth: false,
        );
        vertical.jumpTo(layout.pages[index].rect.top - 24);
        await _settle(tester, cache);
        final object = objects[index - 2];
        final bounds = object.rectangle;
        final projected = edits
            .projectPage(layout: layout.binding, pageIndex: index)
            .single
            .rect;
        final screenRect = tester.getRect(_object(object.id));
        expect(screenRect.size, Size(projected.width, projected.height));
        await tester.tapAt(screenRect.center);
        await tester.pump();
        expect(edits.snapshot().selected, object.id);
        final handle = find.byKey(ValueKey('editor-handle-${object.id}-2'));
        expect(
          tester.getSize(handle),
          const Size(
            EditorObjectOverlay.handleSize,
            EditorObjectOverlay.handleSize,
          ),
        );
        await _mouseDrag(
          tester,
          screenRect.center,
          const Offset(8, 6),
          frames: 5,
        );
        expect(edits.snapshot().undoCount, history + 1);
        await tester.tap(find.byKey(const ValueKey('editor-undo')));
        await tester.pump();
        expect(edits.snapshot().objects[index - 2].rectangle, bounds);
        await _mouseDrag(
          tester,
          tester.getCenter(handle),
          const Offset(5, 3),
          frames: 5,
        );
        expect(edits.snapshot().undoCount, history + 1);
        await tester.tap(find.byKey(const ValueKey('editor-undo')));
        await tester.pump();
        var exactScale = 1.0;
        for (var step = 0; step < 7; step++) {
          await tester.tap(find.byKey(const ValueKey('editor-zoom-in')));
          await tester.pump();
          // Preview rasters may be replaced at a different resolution; geometry is independent.
          await _settle(tester, cache);
          exactScale = (exactScale * 1.25).clamp(0.25, 4.0);
          final current = session.layout(
            workspaceWidth: tester.getSize(workspace).width,
            scale: exactScale,
            fitWidth: false,
          );
          final rect = edits
              .projectPage(layout: current.binding, pageIndex: index)
              .single
              .rect;
          vertical.jumpTo(current.pages[index].rect.top - 24);
          final horizontal = tester
              .widget<SingleChildScrollView>(
                find.byWidgetPredicate(
                  (widget) =>
                      widget is SingleChildScrollView &&
                      widget.controller != null &&
                      widget.scrollDirection == Axis.horizontal,
                ),
              )
              .controller!;
          horizontal.jumpTo(
            (rect.left + rect.width / 2 - tester.getSize(workspace).width / 2)
                .clamp(0, horizontal.position.maxScrollExtent),
          );
          await _settle(tester, cache);
          expect(
            tester.getSize(_object(object.id)).width,
            closeTo(rect.width, 1e-6),
          );
          expect(exactScale, lessThanOrEqualTo(4));
          expect(edits.snapshot().objects, objects);
          expect(edits.snapshot().undoCount, history);
          expect(tester.getSize(handle), const Size(8, 8));
        }
        await tester.tap(find.byKey(const ValueKey('editor-reset-zoom')));
        await _settle(tester, cache);
        vertical.jumpTo(layout.pages[index].rect.top - 24);
        await _settle(tester, cache);
        expect(tester.getRect(_object(object.id)), screenRect);
      }
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets(
    'replacement during a captured gesture discards edits and rejects late rasters',
    (tester) async {
      final old = _DelayedSession(
        await _open(tester, sourcePath: _fixture('two_page.pdf').path),
      );
      final edits = old.createEdits();
      edits.addPrototype(pageIndex: 0);
      final cache = EditorRenderCache();
      await tester.pumpWidget(_app(old, edits, cache));
      await tester.pump();
      final id = edits.snapshot().objects.single.id;
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      final center = tester.getCenter(_object(id));
      await mouse.addPointer(location: center);
      await mouse.down(center);
      await mouse.moveTo(center + const Offset(30, 20));
      await tester.pump();
      expect(edits.snapshot().undoCount, 1);
      final next = await _open(
        tester,
        sourcePath: _fixture('two_page.pdf').path,
      );
      final clean = next.createEdits();
      await tester.pumpWidget(_app(next, clean, cache));
      await tester.pump();
      await mouse.up();
      await mouse.removePointer();
      old.finish();
      await _settle(tester, cache);
      expect(clean.snapshot().objects, isEmpty);
      expect(clean.snapshot().selected, isNull);
      expect(clean.snapshot().undoCount, 0);
      expect(clean.snapshot().redoCount, 0);
      expect(edits.snapshot().undoCount, 1);
      expect(
        cache.cachedKeys.every((key) => key.sessionId == next.identity()),
        isTrue,
      );
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets(
    'editor shortcuts yield to a focused text input and pointer cancellation commits nothing',
    (tester) async {
      final session = await _open(
        tester,
        sourcePath: _fixture('two_page.pdf').path,
      );
      final edits = session.createEdits();
      final cache = EditorRenderCache();
      final text = TextEditingController(text: 'Future input');
      await tester.pumpWidget(
        _app(
          session,
          edits,
          cache,
          input: TextField(
            key: const ValueKey('focus-owner'),
            controller: text,
          ),
        ),
      );
      await _settle(tester, cache);
      await tester.tap(find.byKey(const ValueKey('editor-add-object')));
      await tester.pump();
      final id = edits.snapshot().objects.single.id;
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      final center = tester.getCenter(_object(id));
      await mouse.addPointer(location: center);
      await mouse.down(center);
      await mouse.moveTo(center + const Offset(10, 10));
      await tester.pump();
      await mouse.cancel();
      await mouse.removePointer();
      await tester.pump();
      expect(edits.snapshot().undoCount, 1);
      await tester.tap(find.byKey(const ValueKey('focus-owner')));
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.delete);
      await _shortcut(tester, LogicalKeyboardKey.keyZ);
      expect(edits.snapshot().objects.single.id, id);
      expect(edits.snapshot().undoCount, 1);
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      text.dispose();
      await tester.pump();
    },
  );
}
