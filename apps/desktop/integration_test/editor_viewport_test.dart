import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/editor/editor_render_cache.dart';
import 'package:ilikepdf/src/app/editor/editor_viewport.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

class _TrackingSession implements EditorSession {
  _TrackingSession(this.inner);
  final EditorSession inner;
  final requests = <(int, EditorRasterSize)>[];
  @override
  BigInt identity() => inner.identity();
  @override
  int pageCount() => inner.pageCount();
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
    requests.add((pageIndex, size));
    return inner.render(pageIndex: pageIndex, size: size);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

File _fixture(String name) =>
    File('../../crates/ilikepdf_pdf/tests/fixtures/$name').absolute;
Future<void> _settle(WidgetTester tester, EditorRenderCache cache) async {
  await tester.pump();
  await tester.runAsync(() => cache.idle);
  await tester.pumpAndSettle();
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustLib.init);

  testWidgets(
    'native bridge markers align with layout and scrolled hit testing at every rotation',
    (tester) async {
      final source = _fixture('editor_geometry.pdf');
      final before = await source.readAsBytes();
      final session = await openEditorSession(sourcePath: source.path);
      expect(session.pageCount(), 12);
      for (final scale in [0.25, 1.0, 4.0]) {
        final layout = session.layout(
          workspaceWidth: 900,
          scale: scale,
          fitWidth: false,
        );
        for (final index in [2, 3, 4, 5]) {
          final page = layout.pages[index];
          final size = layout.binding.renderSize(
            pageIndex: index,
            density: 1.5,
          );
          final raster = await session.render(pageIndex: index, size: size);
          final codec = await ui.instantiateImageCodec(raster.png);
          final image = (await codec.getNextFrame()).image;
          final pixels = (await image.toByteData(
            format: ui.ImageByteFormat.rawRgba,
          ))!;
          for (final marker in [
            (const EditorPoint(x: 75, y: 65), [0, 0, 255]),
            (const EditorPoint(x: 175, y: 135), [255, 0, 0]),
          ]) {
            final point = page.transform.pdfToDocument(point: marker.$1);
            final scroll = EditorPoint(x: 17, y: page.rect.top - 23);
            final hit = layout.binding.hitTest(
              point: EditorPoint(x: point.x - scroll.x, y: point.y - scroll.y),
              scroll: scroll,
            )!;
            expect(hit.pageIndex, index);
            expect(hit.pdfPoint.x, closeTo(marker.$1.x, 1e-7));
            expect(hit.pdfPoint.y, closeTo(marker.$1.y, 1e-7));
            final x =
                ((point.x - page.rect.left) / page.rect.width * size.width)
                    .floor();
            final y =
                ((point.y - page.rect.top) / page.rect.height * size.height)
                    .floor();
            final offset = (y * size.width + x) * 4;
            expect([
              pixels.getUint8(offset),
              pixels.getUint8(offset + 1),
              pixels.getUint8(offset + 2),
            ], marker.$2);
          }
          image.dispose();
          codec.dispose();
        }
      }
      expect(await source.readAsBytes(), before);
    },
  );

  testWidgets(
    'desktop viewport scrolls, shares raster bounds, anchors zoom and replaces sessions',
    (tester) async {
      tester.view.resetPhysicalSize();
      tester.view.physicalSize = const Size(1000, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final source = _fixture('editor_geometry.pdf');
      final before = await source.readAsBytes();
      final session = _TrackingSession(
        await openEditorSession(sourcePath: source.path),
      );
      final cache = EditorRenderCache();
      EditorHit? hit;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: EditorViewport(
              session: session,
              cache: cache,
              onHit: (value) => hit = value,
            ),
          ),
        ),
      );
      await _settle(tester, cache);
      expect(session.requests.length, lessThan(12));
      final workspace = find.byKey(const ValueKey('editor-workspace'));
      final width = tester.getSize(workspace).width;
      final layout = session.layout(
        workspaceWidth: width,
        scale: 1,
        fitWidth: true,
      );
      final scroll = tester
          .widget<SingleChildScrollView>(
            find.byKey(const ValueKey('editor-vertical-scroll')),
          )
          .controller!;
      scroll.jumpTo(layout.pages[2].rect.top - 24);
      await _settle(tester, cache);
      final page = layout.pages[2];
      expect(
        tester.getSize(find.byKey(const ValueKey('editor-page-2'))),
        Size(page.rect.width, page.rect.height),
      );
      final marker = page.transform.pdfToDocument(
        point: const EditorPoint(x: 75, y: 65),
      );
      await tester.tapAt(
        tester.getTopLeft(workspace) +
            Offset(marker.x, marker.y - scroll.offset),
      );
      expect(hit!.pageIndex, 2);
      expect(hit!.pdfPoint.x, closeTo(75, 1e-7));
      expect(hit!.pdfPoint.y, closeTo(65, 1e-7));
      final oldCenter = layout.binding.hitTest(
        point: EditorPoint(
          x: width / 2,
          y: tester.getSize(workspace).height / 2,
        ),
        scroll: EditorPoint(x: 0, y: scroll.offset),
      );
      final nextLayout = session.layout(
        workspaceWidth: width,
        scale: layout.scale * 1.25,
        fitWidth: false,
      );
      final expectedScroll = nextLayout.binding.anchoredScroll(
        previous: layout.binding,
        scroll: EditorPoint(x: 0, y: scroll.offset),
        viewport: EditorPoint(x: width, y: tester.getSize(workspace).height),
      );
      double? firstFrameOffset;
      tester.binding.addPostFrameCallback(
        (_) => firstFrameOffset = scroll.offset,
      );
      await tester.tap(find.byKey(const ValueKey('editor-zoom-in')));
      await tester.pump();
      expect(firstFrameOffset, closeTo(expectedScroll.y, 1e-7));
      await _settle(tester, cache);
      final scaleText = tester
          .widget<Text>(find.byKey(const ValueKey('editor-zoom-value')))
          .data!;
      expect(scaleText, '${(layout.scale * 1.25 * 100).round()}%');
      // Zoom preserves the same visible source point when the center is on a page.
      await tester.tapAt(tester.getCenter(workspace));
      if (oldCenter != null) {
        expect(hit!.pageIndex, oldCenter.pageIndex);
        expect(hit!.pdfPoint.y, closeTo(oldCenter.pdfPoint.y, 1e-6));
      }
      expect(
        cache.cachedKeys.every((key) => key.sessionId == session.identity()),
        isTrue,
      );
      await tester.tap(find.byKey(const ValueKey('editor-reset-zoom')));
      await _settle(tester, cache);
      expect(find.text('100%'), findsNWidgets(2));
      await tester.tap(find.byKey(const ValueKey('editor-fit-width')));
      await _settle(tester, cache);
      final replacement = _TrackingSession(
        await openEditorSession(sourcePath: _fixture('two_page.pdf').path),
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: EditorViewport(session: replacement, cache: cache),
          ),
        ),
      );
      await _settle(tester, cache);
      expect(scroll.offset, 0);
      expect(
        cache.cachedKeys.every(
          (key) => key.sessionId == replacement.identity(),
        ),
        isTrue,
      );
      expect(await source.readAsBytes(), before);
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets(
    'A4 native raster covers DPR two at 100, 200, 300 and 400 percent with aligned markers',
    (tester) async {
      final source = _fixture('editor_geometry.pdf');
      final before = await source.readAsBytes();
      final session = await openEditorSession(sourcePath: source.path);
      var previousPixels = 0;
      for (final scale in [1.0, 2.0, 3.0, 4.0]) {
        final layout = session.layout(
          workspaceWidth: 900,
          scale: scale,
          fitWidth: false,
        );
        final page = layout.pages[0];
        final size = layout.binding.renderSize(pageIndex: 0, density: 2);
        expect(size.width, greaterThanOrEqualTo(page.rect.width * 2));
        expect(size.height, greaterThanOrEqualTo(page.rect.height * 2));
        expect(size.width * size.height, greaterThan(previousPixels));
        expect(size.width * size.height, lessThanOrEqualTo(32 * 1024 * 1024));
        previousPixels = size.width * size.height;
        final raster = await session.render(pageIndex: 0, size: size);
        final codec = await ui.instantiateImageCodec(raster.png);
        final image = (await codec.getNextFrame()).image;
        expect(image.width, size.width);
        expect(image.height, size.height);
        final pixels = (await image.toByteData(
          format: ui.ImageByteFormat.rawRgba,
        ))!;
        final point = page.transform.pdfToDocument(
          point: const EditorPoint(x: 75, y: 65),
        );
        final x = ((point.x - page.rect.left) / page.rect.width * size.width)
            .floor();
        final y = ((point.y - page.rect.top) / page.rect.height * size.height)
            .floor();
        final offset = (y * size.width + x) * 4;
        expect(
          [
            pixels.getUint8(offset),
            pixels.getUint8(offset + 1),
            pixels.getUint8(offset + 2),
          ],
          [0, 0, 255],
        );
        expect(layout.pages[0].rect, page.rect);
        image.dispose();
        codec.dispose();
      }
      expect(await source.readAsBytes(), before);
    },
  );

  testWidgets(
    '64-page mixed document renders incrementally with bounded memory through scrolling and zoom',
    (tester) async {
      final session = _TrackingSession(
        await openEditorSession(
          sourcePath: _fixture('editor_viewport_64.pdf').path,
        ),
      );
      final cache = EditorRenderCache();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: EditorViewport(session: session, cache: cache),
          ),
        ),
      );
      await _settle(tester, cache);
      expect(session.pageCount(), 64);
      expect(session.requests.length, lessThan(8));
      final layout = session.layout(
        workspaceWidth: tester
            .getSize(find.byKey(const ValueKey('editor-workspace')))
            .width,
        scale: 1,
        fitWidth: true,
      );
      final scroll = tester
          .widget<SingleChildScrollView>(
            find.byKey(const ValueKey('editor-vertical-scroll')),
          )
          .controller!;
      for (var i = 4; i < 64; i += 4) {
        final before = session.requests.length;
        scroll.jumpTo(
          layout.pages[i].rect.top.clamp(0, scroll.position.maxScrollExtent),
        );
        await _settle(tester, cache);
        expect(session.requests.length, greaterThan(before));
        expect(
          cache.cachedCount,
          lessThanOrEqualTo(EditorRenderCache.maximumEntries),
        );
        expect(
          cache.retainedBytes,
          lessThanOrEqualTo(EditorRenderCache.maximumBytes),
        );
        expect(cache.activeCount, 0);
      }
      for (var i = 0; i < 3; i++) {
        await tester.tap(find.byKey(const ValueKey('editor-zoom-in')));
        await _settle(tester, cache);
      }
      expect(
        cache.cachedCount,
        lessThanOrEqualTo(EditorRenderCache.maximumEntries),
      );
      expect(
        cache.retainedBytes,
        lessThanOrEqualTo(EditorRenderCache.maximumBytes),
      );
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      await tester.pump();
    },
  );
}
