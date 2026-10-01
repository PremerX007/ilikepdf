import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/shared/page_thumbnail_cache.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('rapid source replacements keep the native concurrency bound', () async {
    final oldRender = Completer<String>();
    final old = PageThumbnailCache(render: (_) => oldRender.future);
    final lease = old.acquire(0);
    old.dispose();
    final skipped = PageThumbnailCache(
      render: (_) async => throw StateError('unused'),
      previousSessionIdle: old.disposedAndIdle,
    );
    skipped.dispose();
    var started = false;
    final current = PageThumbnailCache(
      render: (_) async {
        started = true;
        throw StateError('test render');
      },
      previousSessionIdle: skipped.disposedAndIdle,
    );
    final currentLease = current.acquire(0);
    await Future<void>.delayed(Duration.zero);
    expect(started, isFalse);
    oldRender.completeError(StateError('old render'));
    await Future<void>.delayed(Duration.zero);
    expect(started, isTrue);
    old.release(0, lease);
    current.release(0, currentLease);
    current.dispose();
  });

  test('renders at most two, cancels offscreen queue, and discards late disposal results', () async {
    final directory = Directory.systemTemp.createTempSync(
      'thumbnail-cache-test-',
    );
    addTearDown(() => directory.deleteSync(recursive: true));
    final pending = <int, Completer<String>>{};
    final cache = PageThumbnailCache(
      render: (page) => (pending[page] = Completer<String>()).future,
    );
    final leases = [for (var page = 0; page < 20; page++) cache.acquire(page)];
    expect(pending.keys, [0, 1]);
    expect(cache.activeCount, 2);
    for (var page = 2; page < 20; page++) {
      cache.release(page, leases[page]);
    }
    expect(cache.queuedCount, 0);
    cache.dispose();
    for (var page = 0; page < 2; page++) {
      final file = File('${directory.path}/$page.png')..writeAsBytesSync([1]);
      pending[page]!.complete(file.path);
      cache.release(page, leases[page]);
    }
    await Future<void>.delayed(const Duration(milliseconds: 30));
    expect(cache.activeCount, 0);
    expect(directory.listSync(), isEmpty);
  });

  test(
    'LRU retains at most 32 files, reuses entries, and bounds failure state',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'thumbnail-lru-test-',
      );
      addTearDown(() => directory.deleteSync(recursive: true));
      var renders = 0;
      final cache = PageThumbnailCache(
        render: (page) async {
          renders++;
          if (page >= 100) throw StateError('fixture failure');
          return (File(
            '${directory.path}/$page.png',
          )..writeAsBytesSync([1])).path;
        },
      );
      for (var page = 0; page < 50; page++) {
        final lease = cache.acquire(page);
        await Future<void>.delayed(Duration.zero);
        expect(lease.value.path, isNotNull);
        cache.release(page, lease);
        expect(cache.cachedCount, lessThanOrEqualTo(32));
      }
      final reused = cache.acquire(49);
      expect(reused.value.path, isNotNull);
      expect(renders, 50);
      cache.release(49, reused);
      await Future<void>.delayed(const Duration(milliseconds: 30));
      expect(directory.listSync().length, 32);
      expect(File('${directory.path}/0.png').existsSync(), isFalse);
      for (var page = 100; page < 150; page++) {
        final lease = cache.acquire(page);
        await Future<void>.delayed(Duration.zero);
        expect(lease.value.loading, isFalse);
        expect(lease.value.path, isNull);
        cache.release(page, lease);
        expect(cache.cachedCount, lessThanOrEqualTo(32));
      }
      cache.dispose();
    },
  );
}
