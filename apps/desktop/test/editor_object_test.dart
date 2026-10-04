import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/editor/editor_interaction.dart';
import 'package:ilikepdf/src/app/editor/editor_object_overlay.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';

class _Binding implements EditorLayoutBinding {
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Transform implements EditorPageTransform {
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

EditorObjectDisplay _display(double scale) => EditorObjectDisplay(
  id: BigInt.one,
  selected: true,
  rect: EditorRect(
    left: 24 + 20 * scale,
    top: 24 + 30 * scale,
    width: 80 * scale,
    height: 60 * scale,
  ),
  handles: [
    EditorPoint(x: 24 + 20 * scale, y: 24 + 30 * scale),
    EditorPoint(x: 24 + 100 * scale, y: 24 + 90 * scale),
  ],
);

class _Gesture implements EditorObjectGesture {
  int previews = 0;
  @override
  int pageIndex() => 0;
  @override
  EditorObjectDisplay preview({required EditorPoint point}) {
    previews++;
    return _display(1);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Edits implements EditorEdits {
  final gesture = _Gesture();
  int commits = 0;
  @override
  EditorEditSnapshot snapshot() => EditorEditSnapshot(
    sessionId: BigInt.one,
    objects: const [],
    selected: BigInt.one,
    undoCount: commits,
    redoCount: 0,
  );
  @override
  EditorObjectGesture beginGesture({
    required EditorLayoutBinding layout,
    required EditorPoint point,
  }) => gesture;
  @override
  bool finishGesture({
    required EditorObjectGesture gesture,
    required EditorPoint point,
  }) {
    commits++;
    return true;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  test('80 transient frames produce a single commit; cancelled and stale pointers cannot commit', () {
    final edits = _Edits();
    final interaction = EditorInteraction(edits);
    const point = EditorPoint(x: 30, y: 40);
    interaction.begin(_Binding(), point, 1);
    for (var i = 0; i < 80; i++) {
      interaction.update(point, 1);
    }
    expect(edits.gesture.previews, 80);
    expect(edits.commits, 0);
    expect(interaction.preview, isNotNull);
    interaction.finish(point, 1);
    expect(edits.commits, 1);
    expect(interaction.preview, isNull);
    interaction.finish(point, 1);
    expect(edits.commits, 1);
    interaction.begin(_Binding(), point, 2);
    interaction.update(point, 2);
    interaction.cancel();
    interaction.finish(point, 2);
    expect(edits.commits, 1);
    interaction.begin(_Binding(), point, 3);
    interaction.finish(point, 2);
    expect(edits.commits, 1);
    interaction.dispose();
  });
  testWidgets(
    'overlay uses projected page placement and constant logical handles across zoom and rebuilds',
    (tester) async {
      for (final scale in [0.25, 1.0, 2.0, 4.0, 1.0]) {
        final page = EditorPageLayout(
          pageIndex: 0,
          rect: EditorRect(
            left: 24,
            top: 24,
            width: 150 * scale,
            height: 120 * scale,
          ),
          scale: scale,
          geometry: const EditorPageGeometry(
            visibleBox: EditorPageBox(
              left: 25,
              bottom: 30,
              right: 175,
              top: 150,
            ),
            rotation: EditorPageRotation.clockwise90,
          ),
          transform: _Transform(),
        );
        Widget app() => MaterialApp(
          home: Align(
            alignment: Alignment.topLeft,
            child: SizedBox(
              width: page.rect.width,
              height: page.rect.height,
              child: EditorObjectOverlay(
                page: page,
                objects: [_display(scale)],
              ),
            ),
          ),
        );
        await tester.pumpWidget(app());
        final rect = tester.getRect(
          find.byKey(const ValueKey('editor-object-1')),
        );
        expect(
          rect,
          Rect.fromLTWH(20 * scale, 30 * scale, 80 * scale, 60 * scale),
        );
        expect(
          tester.getSize(find.byKey(const ValueKey('editor-handle-1-0'))),
          const Size(8, 8),
        );
        await tester.pumpWidget(app());
        expect(
          tester.getRect(find.byKey(const ValueKey('editor-object-1'))),
          rect,
        );
      }
    },
  );
}
