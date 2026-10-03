import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/editor/editor_render_cache.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

class _Session implements EditorSession {
  _Session(this.id, this.renderPage);
  final int id;
  final Future<EditorRaster> Function(int, EditorRasterSize) renderPage;
  @override
  BigInt identity() => BigInt.from(id);
  @override
  Future<EditorRaster> render({
    required int pageIndex,
    required EditorRasterSize size,
  }) => renderPage(pageIndex, size);
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

EditorRenderKey _key(
  int page, {
  int session = 1,
  int width = 20,
  int height = 30,
}) => EditorRenderKey(
  BigInt.from(session),
  page,
  EditorRasterSize(width: width, height: height),
);
EditorRaster _raster(EditorRasterSize size) {
  final data = ByteData(8)
    ..setUint32(0, size.width)
    ..setUint32(4, size.height);
  return EditorRaster(
    png: data.buffer.asUint8List(),
    width: size.width,
    height: size.height,
  );
}

Future<ui.Image> _decode(Uint8List bytes) async {
  final data = ByteData.sublistView(bytes);
  final recorder = ui.PictureRecorder();
  ui.Canvas(recorder).drawColor(const ui.Color(0xFF0000FF), ui.BlendMode.src);
  final picture = recorder.endRecording();
  try {
    return await picture.toImage(data.getUint32(0), data.getUint32(4));
  } finally {
    picture.dispose();
  }
}

void main() {
  testWidgets(
    'renders only demand, bounds concurrency, and reuses exact rasters',
    (tester) async {
      final requests = <int>[];
      final pending = <int, Completer<EditorRaster>>{};
      final cache = EditorRenderCache(decode: _decode)
        ..bind(
          _Session(1, (page, size) {
            requests.add(page);
            return (pending[page] = Completer<EditorRaster>()).future;
          }),
        );
      expect(requests, isEmpty);
      cache.demand([_key(0), _key(1), _key(2)]);
      expect(requests, [0, 1]);
      expect(cache.activeCount, 2);
      pending[0]!.complete(_raster(_key(0).size));
      await tester.runAsync(() async {
        await Future<void>.delayed(const Duration(milliseconds: 40));
      });
      expect(requests, [0, 1, 2]);
      pending[1]!.complete(_raster(_key(1).size));
      pending[2]!.complete(_raster(_key(2).size));
      await tester.runAsync(() => cache.idle);
      cache.demand([_key(0), _key(1), _key(2)]);
      expect(requests.length, 3);
      expect(cache.state(_key(0)).exact, isTrue);
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets(
    'zoom keeps a temporary raster and rejects old in-flight resolutions',
    (tester) async {
      final pending = <EditorRasterSize, Completer<EditorRaster>>{};
      final cache = EditorRenderCache(decode: _decode)
        ..bind(
          _Session(
            1,
            (page, size) => (pending[size] = Completer<EditorRaster>()).future,
          ),
        );
      cache.demand([_key(0)]);
      pending[_key(0).size]!.complete(_raster(_key(0).size));
      await tester.runAsync(() => cache.idle);
      cache.invalidateResolution();
      cache.demand([_key(0, width: 40, height: 60)]);
      expect(cache.state(_key(0, width: 40, height: 60)).image, isNotNull);
      expect(cache.state(_key(0, width: 40, height: 60)).exact, isFalse);
      cache.invalidateResolution();
      cache.demand([_key(0, width: 60, height: 90)]);
      pending[_key(0, width: 40, height: 60).size]!.complete(
        _raster(_key(0, width: 40, height: 60).size),
      );
      pending[_key(0, width: 60, height: 90).size]!.complete(
        _raster(_key(0, width: 60, height: 90).size),
      );
      await tester.runAsync(() => cache.idle);
      expect(cache.cachedKeys, {_key(0, width: 60, height: 90)});
      expect(cache.retainedBytes, 60 * 90 * 4);
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets(
    'session replacement and close discard late results without extra workers',
    (tester) async {
      final old = Completer<EditorRaster>();
      var newCalls = 0;
      final cache = EditorRenderCache(decode: _decode)
        ..bind(_Session(1, (page, size) => old.future));
      cache.demand([_key(0)]);
      cache.bind(
        _Session(2, (page, size) async {
          newCalls++;
          return _raster(size);
        }),
      );
      cache.demand([_key(0, session: 2)]);
      old.complete(_raster(_key(0).size));
      await tester.runAsync(() => cache.idle);
      expect(newCalls, 1);
      expect(cache.cachedKeys, {_key(0, session: 2)});
      cache.bind(null);
      expect(cache.cachedCount, 0);
      expect(cache.retainedBytes, 0);
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets('evicts decoded images and failures across a long document', (
    tester,
  ) async {
    var calls = 0;
    final cache = EditorRenderCache(decode: _decode)
      ..bind(
        _Session(1, (page, size) async {
          calls++;
          return _raster(size);
        }),
      );
    for (var page = 0; page < 64; page++) {
      cache.demand([_key(page)]);
      await tester.runAsync(() => cache.idle);
      expect(
        cache.cachedCount,
        lessThanOrEqualTo(EditorRenderCache.maximumEntries),
      );
      expect(
        cache.retainedBytes,
        lessThanOrEqualTo(EditorRenderCache.maximumBytes),
      );
      await tester.pump();
    }
    expect(calls, 64);
    expect(cache.cachedKeys.contains(_key(0)), isFalse);
    cache.invalidateResolution();
    cache.demand([_key(63, width: 40, height: 60)]);
    await tester.runAsync(() => cache.idle);
    expect(cache.cachedKeys, {_key(63, width: 40, height: 60)});
    cache.dispose();
    await tester.pump();
  });

  testWidgets('byte budget limits high resolution demand before rendering', (
    tester,
  ) async {
    var calls = 0;
    final cache = EditorRenderCache(decode: _decode)
      ..bind(
        _Session(1, (page, size) async {
          calls++;
          return _raster(size);
        }),
      );
    cache.demand(List.generate(20, (i) => _key(i, width: 4000, height: 4000)));
    await tester.runAsync(() => cache.idle);
    expect(calls, 4);
    expect(cache.cachedCount, 4);
    expect(cache.retainedBytes, 256_000_000);
    cache.dispose();
    await tester.pump();
  });

  testWidgets(
    'rapid resolution changes coalesce, bound active bytes, and skip stale decode',
    (tester) async {
      final pending = <EditorRenderKey, Completer<EditorRaster>>{};
      var decodes = 0;
      final cache =
          EditorRenderCache(
            decode: (bytes) async {
              decodes++;
              return _decode(bytes);
            },
          )..bind(
            _Session(1, (page, size) {
              final key = EditorRenderKey(BigInt.one, page, size);
              return (pending[key] = Completer<EditorRaster>()).future;
            }),
          );
      final large = _key(0, width: 8192, height: 4096);
      cache.demand([large, _key(1)]);
      expect(cache.activeBytes, EditorRenderCache.maximumActiveBytes);
      expect(cache.activeCount, 1);
      for (var width = 40; width <= 200; width += 20) {
        cache.invalidateResolution();
        cache.demand([_key(0, width: width)]);
        expect(cache.queuedCount, 1);
        expect(cache.activeCount, 1);
      }
      pending[large]!.complete(_raster(large.size));
      await tester.pump();
      final finalKey = _key(0, width: 200);
      expect(pending.keys, [large, finalKey]);
      expect(decodes, 0);
      // Same physical size after a tiny fit-width change reuses the active job.
      cache.invalidateResolution();
      cache.demand([finalKey]);
      pending[finalKey]!.complete(_raster(finalKey.size));
      await tester.runAsync(() => cache.idle);
      expect(pending.length, 2);
      expect(decodes, 1);
      expect(cache.cachedKeys, {finalKey});
      expect(cache.activeBytes, 0);
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets(
    'one failed page stays bounded, preserves neighbors, and retries explicitly',
    (tester) async {
      var fail = true;
      var failures = 0;
      final cache = EditorRenderCache(decode: _decode)
        ..bind(
          _Session(1, (page, size) async {
            if (page == 1 && fail) {
              failures++;
              throw const ApplicationError(
                code: ApplicationErrorCode.renderingFailed,
                message: 'This page could not be rendered.',
              );
            }
            return _raster(size);
          }),
        );
      cache.demand([_key(0), _key(1)]);
      await tester.runAsync(() => cache.idle);
      expect(cache.state(_key(0)).image, isNotNull);
      expect(cache.state(_key(1)).error, isNotNull);
      cache.demand([_key(0), _key(1)]);
      expect(failures, 1);
      fail = false;
      cache.retry(_key(1));
      await tester.runAsync(() => cache.idle);
      expect(cache.state(_key(1)).image, isNotNull);
      cache.dispose();
      await tester.pump();
    },
  );

  testWidgets('disposal during decoding disposes the rejected image', (
    tester,
  ) async {
    final decode = Completer<ui.Image>();
    final cache = EditorRenderCache(decode: (_) => decode.future)
      ..bind(_Session(1, (page, size) async => _raster(size)));
    cache.demand([_key(0)]);
    await tester.pump();
    cache.dispose();
    final image = await tester.runAsync(
      () => _decode(_raster(_key(0).size).png),
    );
    decode.complete(image!);
    await tester.runAsync(() => cache.idle);
    expect(image.debugDisposed, isTrue);
  });
}
