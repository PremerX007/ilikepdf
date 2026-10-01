import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/shared/file_preview_card.dart';
import 'package:ilikepdf/src/app/shared/reorderable_item_grid.dart';

void main() {
  for (final lazy in [false, true]) {
    final variant = lazy ? 'lazy' : 'document';
    testWidgets('$variant grid flows across rows before committing on drop', (
      tester,
    ) async {
      final harness = await _pumpGrid(tester, lazy: lazy);
      final original = List.generate(6, (id) => tester.getTopLeft(_card(id)));
      final retainedState = tester.state(_card(1));
      final gesture = await _startDrag(tester, 0);
      await gesture.moveTo(tester.getCenter(_slot(lazy, 4)));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 60));
      final moving = tester.getTopLeft(_card(1));
      expect(moving.dx, greaterThan(original[0].dx));
      expect(moving.dx, lessThan(original[1].dx));
      await tester.pump(const Duration(milliseconds: 120));
      for (var id = 1; id <= 4; id++) {
        expect(tester.getTopLeft(_card(id)), original[id - 1]);
      }
      expect(tester.getTopLeft(_card(5)), original[5]);
      expect(
        tester.getTopLeft(find.byKey(const ValueKey('reorder-placeholder'))),
        original[4],
      );
      expect(harness.currentState!.items, [0, 1, 2, 3, 4, 5]);
      expect(harness.currentState!.reorderCalls, 0);

      // Moving back uses fixed grid slots, even while cards are sliding.
      await gesture.moveTo(tester.getCenter(_slot(lazy, 1)));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(tester.getTopLeft(_card(1)), original[0]);
      expect(tester.getTopLeft(_card(2)), original[2]);
      expect(
        tester.getTopLeft(find.byKey(const ValueKey('reorder-placeholder'))),
        original[1],
      );
      await gesture.up();
      await tester.pumpAndSettle();
      expect(harness.currentState!.items, [1, 0, 2, 3, 4, 5]);
      expect(harness.currentState!.reorderCalls, 1);
      expect(tester.state(_card(1)), same(retainedState));
      expect(tester.getTopLeft(_card(1)), original[0]);
      expect(find.byKey(const ValueKey('reorder-placeholder')), findsNothing);
    });

    testWidgets(
      '$variant canceled drag restores the original positions and order',
      (tester) async {
        final harness = await _pumpGrid(tester, lazy: lazy);
        final original = List.generate(6, (id) => tester.getTopLeft(_card(id)));
        final gesture = await _startDrag(tester, 4);
        await gesture.moveTo(tester.getCenter(_slot(lazy, 0)));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 200));
        expect(tester.getTopLeft(_card(0)), original[1]);
        await gesture.cancel();
        await tester.pumpAndSettle();
        expect(harness.currentState!.items, [0, 1, 2, 3, 4, 5]);
        expect(harness.currentState!.reorderCalls, 0);
        for (var id = 0; id < 6; id++) {
          expect(tester.getTopLeft(_card(id)), original[id]);
        }
        expect(find.byKey(const ValueKey('reorder-card-proxy')), findsNothing);
        expect(find.byKey(const ValueKey('reorder-placeholder')), findsNothing);
      },
    );

    for (final surface in ['thumbnail', 'filename', 'empty card area']) {
      testWidgets(
        '$variant card drags from $surface with its contents in the proxy',
        (tester) async {
          final harness = await _pumpGrid(tester, lazy: lazy);
          final start = switch (surface) {
            'thumbnail' => tester.getCenter(
              find.byKey(const ValueKey('thumbnail-0')),
            ),
            'filename' => tester.getCenter(find.text('file0.pdf')),
            _ => tester.getTopLeft(_card(0)) + const Offset(5, 80),
          };
          final gesture = await tester.startGesture(
            start,
            kind: PointerDeviceKind.mouse,
          );
          await gesture.moveBy(const Offset(24, 0));
          await tester.pump();
          final proxy = find.byKey(const ValueKey('reorder-card-proxy'));
          expect(proxy, findsOne);
          expect(
            find.descendant(of: proxy, matching: find.text('file0.pdf')),
            findsOne,
          );
          expect(
            find.descendant(of: proxy, matching: find.text('Preview 0')),
            findsOne,
          );
          await gesture.moveTo(tester.getCenter(_slot(lazy, 2)));
          await tester.pump();
          await gesture.up();
          await tester.pumpAndSettle();
          expect(harness.currentState!.items, [1, 2, 0, 3, 4, 5]);
        },
      );
    }

    testWidgets(
      '$variant card clicks and remove controls never start reordering',
      (tester) async {
        final harness = await _pumpGrid(tester, lazy: lazy);
        final click = await tester.startGesture(
          tester.getCenter(_card(0)),
          kind: PointerDeviceKind.mouse,
        );
        await tester.pump(const Duration(milliseconds: 300));
        expect(find.byKey(const ValueKey('reorder-card-proxy')), findsNothing);
        await click.up();
        await tester.pumpAndSettle();
        final action = find.byKey(const ValueKey('remove-0'));
        final drag = await tester.startGesture(
          tester.getCenter(action),
          kind: PointerDeviceKind.mouse,
        );
        await drag.moveTo(tester.getCenter(_slot(lazy, 2)));
        await tester.pump();
        expect(find.byKey(const ValueKey('reorder-card-proxy')), findsNothing);
        await drag.up();
        await tester.pumpAndSettle();
        expect(harness.currentState!.items, [0, 1, 2, 3, 4, 5]);
        expect(harness.currentState!.reorderCalls, 0);
        await tester.tap(action);
        await tester.pumpAndSettle();
        expect(harness.currentState!.items, [1, 2, 3, 4, 5]);
        expect(harness.currentState!.reorderCalls, 0);
      },
    );

    testWidgets(
      '$variant disabled and secondary mouse drags leave the order intact',
      (tester) async {
        final harness = await _pumpGrid(tester, lazy: lazy);
        final secondary = await tester.startGesture(
          tester.getCenter(_card(0)),
          kind: PointerDeviceKind.mouse,
          buttons: kSecondaryButton,
        );
        await secondary.moveTo(tester.getCenter(_slot(lazy, 2)));
        await secondary.up();
        await tester.pumpAndSettle();
        expect(harness.currentState!.reorderCalls, 0);
        harness.currentState!.setEnabled(false);
        await tester.pump();
        final gesture = await _startDrag(tester, 0);
        expect(find.byKey(const ValueKey('reorder-card-proxy')), findsNothing);
        await gesture.moveTo(tester.getCenter(_slot(lazy, 2)));
        await gesture.up();
        await tester.pumpAndSettle();
        expect(harness.currentState!.items, [0, 1, 2, 3, 4, 5]);
      },
    );

    testWidgets('$variant changed item set invalidates a pending drop', (
      tester,
    ) async {
      final harness = await _pumpGrid(tester, lazy: lazy);
      final gesture = await _startDrag(tester, 0);
      await gesture.moveTo(tester.getCenter(_slot(lazy, 4)));
      await tester.pump();
      harness.currentState!.remove(1);
      await tester.pump();
      await gesture.up();
      await tester.pumpAndSettle();
      expect(harness.currentState!.items, [0, 2, 3, 4, 5]);
      expect(harness.currentState!.reorderCalls, 0);
    });
  }

  for (final cancel in [false, true]) {
    testWidgets(
      'lazy long-distance drag stays bounded and ${cancel ? 'cancels' : 'drops'} after the source leaves the viewport',
      (tester) async {
        final harness = await _pumpGrid(tester, lazy: true, count: 100);
        expect(find.byType(FilePreviewCard).evaluate().length, lessThan(24));
        final grid = find.byKey(const ValueKey('lazy-reorderable-item-grid'));
        final gesture = await _startDrag(tester, 0);
        final edge = tester.getBottomLeft(grid) + const Offset(60, -12);
        for (var step = 0; step < 20; step++) {
          await gesture.moveTo(edge + Offset(step.isEven ? 0 : 1, 0));
          await tester.pump(const Duration(milliseconds: 30));
        }
        expect(
          tester.widget<GridView>(grid).controller!.offset,
          greaterThan(360),
        );
        // Includes the drag proxy, with at most the viewport and nearby cache rows.
        expect(find.byType(FilePreviewCard).evaluate().length, lessThan(24));
        expect(_card(0), findsNothing);
        if (cancel) {
          await gesture.cancel();
        } else {
          await gesture.up();
        }
        await tester.pumpAndSettle();
        expect(harness.currentState!.reorderCalls, cancel ? 0 : 1);
        expect(
          harness.currentState!.items.indexOf(0),
          cancel ? 0 : greaterThan(8),
        );
        expect(find.byKey(const ValueKey('reorder-placeholder')), findsNothing);
        expect(find.byKey(const ValueKey('reorder-card-proxy')), findsNothing);
      },
    );
  }
}

Finder _card(int id) => find.byKey(ValueKey('card-$id'));
Finder _slot(bool lazy, int index) =>
    find.byKey(ValueKey('${lazy ? 'lazy-' : ''}reorder-target-$index'));

Future<TestGesture> _startDrag(WidgetTester tester, int id) async {
  final gesture = await tester.startGesture(
    tester.getCenter(find.byKey(ValueKey('thumbnail-$id'))),
    kind: PointerDeviceKind.mouse,
  );
  await gesture.moveBy(const Offset(24, 0));
  await tester.pump();
  return gesture;
}

Future<GlobalKey<_GridHarnessState>> _pumpGrid(
  WidgetTester tester, {
  required bool lazy,
  int count = 6,
}) async {
  final key = GlobalKey<_GridHarnessState>();
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 420,
            height: 500,
            child: _GridHarness(key: key, lazy: lazy, count: count),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return key;
}

class _GridHarness extends StatefulWidget {
  const _GridHarness({required this.lazy, required this.count, super.key});
  final bool lazy;
  final int count;
  @override
  State<_GridHarness> createState() => _GridHarnessState();
}

class _GridHarnessState extends State<_GridHarness> {
  late final List<int> items = List.generate(widget.count, (index) => index);
  int reorderCalls = 0;
  bool enabled = true;
  void setEnabled(bool value) => setState(() => enabled = value);
  void remove(int id) => setState(() => items.remove(id));
  void reorder(int from, int to) => setState(() {
    reorderCalls++;
    items.insert(to, items.removeAt(from));
  });

  Widget buildCard(
    BuildContext context,
    int item,
    int index,
    ReorderableDragBuilder dragSurface,
  ) => FilePreviewCard(
    key: ValueKey('card-$item'),
    thumbnail: SizedBox.expand(
      key: ValueKey('thumbnail-$item'),
      child: Center(child: Text('Preview $item')),
    ),
    filename: 'file$item.pdf',
    positionLabel: 'Position ${index + 1}',
    dragSurface: dragSurface,
    removeButtonKey: ValueKey('remove-$item'),
    onRemove: () => remove(item),
  );

  @override
  Widget build(BuildContext context) => widget.lazy
      ? LazyReorderableItemGrid<int>(
          items: items,
          itemKey: (item) => ValueKey(item),
          itemBuilder: buildCard,
          onReorder: reorder,
          enabled: enabled,
          maximumItemWidth: 140,
          itemHeight: 150,
          padding: EdgeInsets.zero,
        )
      : ReorderableItemGrid<int>(
          items: items,
          itemKey: (item) => ValueKey(item),
          itemBuilder: buildCard,
          onReorder: reorder,
          enabled: enabled,
          minimumItemWidth: 120,
          itemHeight: 150,
        );
}
