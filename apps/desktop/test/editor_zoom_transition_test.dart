import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/editor/editor_render_cache.dart';
import 'package:ilikepdf/src/app/editor/editor_scroll_controller.dart';
import 'package:ilikepdf/src/app/editor/editor_viewport.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';
import 'package:ilikepdf/src/rust/api/error.dart';
import 'package:ilikepdf/src/rust/frb_generated.dart';

// Deliberately scripted core outputs: this checks Flutter's publication timing,
// independently of the native geometry/anchor tests.
class _LayoutBinding implements EditorLayoutBinding {
  _LayoutBinding(this.scale);
  final double scale;
  @override
  EditorPoint anchoredScroll({
    required EditorLayoutBinding previous,
    required EditorPoint scroll,
    required EditorPoint viewport,
  }) {
    final old = (previous as _LayoutBinding).scale;
    return EditorPoint(
      x: (scroll.x + viewport.x / 2) * scale / old - viewport.x / 2,
      y: (scroll.y + viewport.y / 2) * scale / old - viewport.y / 2,
    );
  }

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
  }) => EditorRasterSize(
    width: (100 * scale * density).round(),
    height: (200 * scale * density).round(),
  );
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Transform implements EditorPageTransform {
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Session implements EditorSession {
  final pending = <Completer<EditorRaster>>[];
  final sizes = <EditorRasterSize>[];
  final scales = <double>[];
  @override
  BigInt identity() => BigInt.one;
  @override
  EditorDocumentLayout layout({
    required double workspaceWidth,
    required double scale,
    required bool fitWidth,
  }) {
    final resolved = fitWidth ? workspaceWidth / 800 : scale;
    scales.add(resolved);
    return EditorDocumentLayout(
      binding: _LayoutBinding(resolved),
      width: 1000 * resolved,
      height: 3000 * resolved,
      scale: resolved,
      pages: [
        EditorPageLayout(
          pageIndex: 0,
          rect: EditorRect(
            left: 100 * resolved,
            top: 1000 * resolved,
            width: 600 * resolved,
            height: 800 * resolved,
          ),
          scale: resolved,
          geometry: const EditorPageGeometry(
            visibleBox: EditorPageBox(left: 0, bottom: 0, right: 600, top: 800),
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
  }) {
    final result = Completer<EditorRaster>();
    sizes.add(size);
    pending.add(result);
    return result.future;
  }

  void finish() {
    for (final result in pending) {
      if (!result.isCompleted) {
        result.completeError(
          const ApplicationError(
            code: ApplicationErrorCode.renderingFailed,
            message: 'Controlled test failure',
          ),
        );
      }
    }
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  setUpAll(RustLib.init);
  testWidgets(
    'DPR-only changes and no-op zoom preserve geometry and ordinary scroll',
    (tester) async {
      final session = _Session();
      final cache = EditorRenderCache();
      Widget app(double density) => MaterialApp(
        home: Scaffold(
          body: MediaQuery(
            data: MediaQueryData(devicePixelRatio: density),
            child: EditorViewport(session: session, cache: cache),
          ),
        ),
      );
      await tester.pumpWidget(app(1));
      final scroll =
          tester
                  .widget<SingleChildScrollView>(
                    find.byKey(const ValueKey('editor-vertical-scroll')),
                  )
                  .controller!
              as EditorScrollController;
      scroll.jumpTo(900);
      await tester.pump();
      final rect = tester.getRect(find.byKey(const ValueKey('editor-page-0')));
      final layouts = session.scales.length;
      await tester.pumpWidget(app(2));
      expect(session.scales.length, layouts);
      expect(tester.getRect(find.byKey(const ValueKey('editor-page-0'))), rect);
      expect(session.sizes.last.width, session.sizes.first.width * 2);
      expect(scroll.offset, 900);
      await tester.tap(find.byKey(const ValueKey('editor-reset-zoom')));
      await tester.pump();
      scroll.jumpTo(950);
      await tester.pump();
      expect(scroll.layoutOffset, 950);
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      session.finish();
      await tester.pump();
    },
  );
  testWidgets('first zoom frame uses new layout and anchored offset together', (
    tester,
  ) async {
    final session = _Session();
    final cache = EditorRenderCache();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: EditorViewport(session: session, cache: cache),
        ),
      ),
    );
    await tester.pump();
    final scroll = tester
        .widget<SingleChildScrollView>(
          find.byKey(const ValueKey('editor-vertical-scroll')),
        )
        .controller!;
    scroll.jumpTo(900);
    await tester.pump();
    final workspace = find.byKey(const ValueKey('editor-workspace'));
    final height = tester.getSize(workspace).height;
    final expectedScroll = (900 + height / 2) * 1.25 - height / 2;
    double? firstFrameScroll;
    double? firstFrameTop;
    // Registered before the click: observes the frame BEFORE any correction
    // callback that the zoom handler might schedule after painting.
    tester.binding.addPostFrameCallback((_) {
      firstFrameScroll = scroll.offset;
      firstFrameTop =
          tester.getTopLeft(find.byKey(const ValueKey('editor-page-0'))).dy -
          tester.getTopLeft(workspace).dy;
    });
    await tester.tap(find.byKey(const ValueKey('editor-zoom-in')));
    await tester.pump();
    expect(firstFrameScroll, closeTo(expectedScroll, 1e-8));
    expect(firstFrameTop, closeTo(1250 - expectedScroll, 1e-8));
    await tester.pumpWidget(const SizedBox());
    cache.dispose();
    session.finish();
    await tester.pump();
  });

  testWidgets(
    'rapid +/- publishes both anchored axes on the first frame and stays stable',
    (tester) async {
      final session = _Session();
      final cache = EditorRenderCache();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: EditorViewport(session: session, cache: cache),
          ),
        ),
      );
      final views = tester.widgetList<SingleChildScrollView>(
        find.byType(SingleChildScrollView),
      );
      final horizontal = views
          .firstWhere((v) => v.scrollDirection == Axis.horizontal)
          .controller!;
      final vertical = views
          .firstWhere((v) => v.scrollDirection == Axis.vertical)
          .controller!;
      horizontal.jumpTo(100);
      vertical.jumpTo(900);
      await tester.pump();
      final workspace = find.byKey(const ValueKey('editor-workspace'));
      final size = tester.getSize(workspace);
      for (final increase in [true, true, true, false]) {
        tester
            .widget<IconButton>(
              find.byKey(
                ValueKey(increase ? 'editor-zoom-in' : 'editor-zoom-out'),
              ),
            )
            .onPressed!();
      }
      final expected = Offset(
        (100 + size.width / 2) * 1.5625 - size.width / 2,
        (900 + size.height / 2) * 1.5625 - size.height / 2,
      );
      for (var frame = 0; frame < 4; frame++) {
        await tester.pump();
        expect(horizontal.offset, closeTo(expected.dx, 1e-8));
        expect(vertical.offset, closeTo(expected.dy, 1e-8));
        expect(
          tester.getTopLeft(find.byKey(const ValueKey('editor-page-0'))) -
              tester.getTopLeft(workspace),
          Offset(156.25 - expected.dx, 1562.5 - expected.dy),
        );
      }
      // Several opposite clicks can arrive before another frame.
      for (final increase in [false, false, true]) {
        tester
            .widget<IconButton>(
              find.byKey(
                ValueKey(increase ? 'editor-zoom-in' : 'editor-zoom-out'),
              ),
            )
            .onPressed!();
      }
      await tester.pump();
      expect(
        vertical.offset,
        closeTo((900 + size.height / 2) * 1.25 - size.height / 2, 1e-8),
      );
      expect(cache.activeCount, lessThanOrEqualTo(2));
      expect(cache.queuedCount, lessThanOrEqualTo(1));
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      session.finish();
      await tester.pump();
    },
  );

  testWidgets(
    'sharper raster replaces fallback without changing rectangle, extents or layout',
    (tester) async {
      final session = _Session();
      final cache = EditorRenderCache(
        decode: (bytes) async {
          final data = ByteData.sublistView(bytes);
          final recorder = ui.PictureRecorder();
          ui.Canvas(recorder).drawColor(Colors.white, ui.BlendMode.src);
          final picture = recorder.endRecording();
          try {
            return await picture.toImage(data.getUint32(0), data.getUint32(4));
          } finally {
            picture.dispose();
          }
        },
      );
      void complete(int index) {
        final size = session.sizes[index];
        final data = ByteData(8)
          ..setUint32(0, size.width)
          ..setUint32(4, size.height);
        session.pending[index].complete(
          EditorRaster(
            png: data.buffer.asUint8List(),
            width: size.width,
            height: size.height,
          ),
        );
      }

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: EditorViewport(session: session, cache: cache),
          ),
        ),
      );
      complete(0);
      await tester.runAsync(() => cache.idle);
      await tester.pump();
      final rasterFinder = find.byKey(const ValueKey('editor-raster-0'));
      final low = tester.widget<RawImage>(rasterFinder).image;
      final scroll = tester
          .widget<SingleChildScrollView>(
            find.byKey(const ValueKey('editor-vertical-scroll')),
          )
          .controller!;
      scroll.jumpTo(900);
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('editor-zoom-in')));
      await tester.pump();
      expect(tester.widget<RawImage>(rasterFinder).image, same(low));
      final pageFinder = find.byKey(const ValueKey('editor-page-0'));
      final rect = tester.getRect(pageFinder);
      final offset = scroll.offset;
      final extent = scroll.position.maxScrollExtent;
      final layouts = session.scales.length;
      complete(1);
      await tester.runAsync(() => cache.idle);
      await tester.pump();
      final high = tester.widget<RawImage>(rasterFinder);
      expect(high.image, isNot(same(low)));
      expect(high.image!.width, greaterThan(low!.width));
      expect(high.filterQuality, FilterQuality.low);
      expect(tester.getRect(pageFinder), rect);
      expect(scroll.offset, offset);
      expect(scroll.position.maxScrollExtent, extent);
      expect(session.scales.length, layouts);
      await tester.pumpWidget(const SizedBox());
      cache.dispose();
      session.finish();
      await tester.pump();
    },
  );

  testWidgets('resize fit width updates label and anchor in the same frame', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final session = _Session();
    final cache = EditorRenderCache();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: EditorViewport(session: session, cache: cache),
        ),
      ),
    );
    final scroll = tester
        .widget<SingleChildScrollView>(
          find.byKey(const ValueKey('editor-vertical-scroll')),
        )
        .controller!;
    scroll.jumpTo(900);
    await tester.pump();
    tester.view.physicalSize = const Size(1000, 700);
    await tester.pump();
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('editor-zoom-value'))).data,
      '125%',
    );
    expect(scroll.offset, closeTo((900 + 320) * 1.25 - 320, 1e-8));
    await tester.pumpWidget(const SizedBox());
    cache.dispose();
    session.finish();
    await tester.pump();
  });
}
