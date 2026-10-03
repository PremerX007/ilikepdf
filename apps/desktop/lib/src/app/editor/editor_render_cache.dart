import 'dart:async';
import 'dart:collection';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/scheduler.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

@immutable
class EditorRenderKey {
  const EditorRenderKey(this.sessionId, this.pageIndex, this.size);
  final BigInt sessionId;
  final int pageIndex;
  final EditorRasterSize size;
  int get bytes => size.width * size.height * 4;
  @override
  int get hashCode => Object.hash(sessionId, pageIndex, size);
  @override
  bool operator ==(Object other) =>
      other is EditorRenderKey &&
      sessionId == other.sessionId &&
      pageIndex == other.pageIndex &&
      size == other.size;
}

class EditorRasterState {
  const EditorRasterState({
    this.image,
    this.error,
    this.exact = false,
    this.loading = true,
  });
  final ui.Image? image;
  final String? error;
  final bool exact;
  final bool loading;
}

/// Owns decoded images directly, avoiding Flutter's unbounded live ImageCache.
/// One scheduler survives session/zoom replacement, including in-flight work.
class EditorRenderCache extends ChangeNotifier {
  EditorRenderCache({this.decode = decodePng});
  static const maximumBytes = 256 * 1024 * 1024;
  static const maximumActiveBytes = 128 * 1024 * 1024;
  static const maximumEntries = 12;
  static const maximumConcurrent = 2;
  final Future<ui.Image> Function(Uint8List) decode;
  final _entries = <EditorRenderKey, EditorRasterState>{};
  final _active = <EditorRenderKey>{};
  final _queue = Queue<EditorRenderKey>();
  final _idleWaiters = <Completer<void>>[];
  Set<EditorRenderKey> _desired = {};
  BigInt? _sessionId;
  Future<EditorRaster> Function(EditorRenderKey)? _render;
  int _generation = 0;
  int _bytes = 0;
  bool _disposed = false;
  bool _resolutionChanged = false;

  int get cachedCount => _entries.length;
  int get retainedBytes => _bytes;
  int get activeCount => _active.length;
  int get activeBytes => _active.fold(0, (bytes, key) => bytes + key.bytes);
  int get queuedCount => _queue.length;
  Set<EditorRenderKey> get cachedKeys => Set.unmodifiable(_entries.keys);
  Future<void> get idle {
    if (_active.isEmpty && _queue.isEmpty) return Future.value();
    final waiter = Completer<void>();
    _idleWaiters.add(waiter);
    return waiter.future;
  }

  void bind(EditorSession? session) {
    final identity = session?.identity();
    if (identity == _sessionId) return;
    _generation++;
    _sessionId = identity;
    _render = session == null
        ? null
        : (key) => session.render(pageIndex: key.pageIndex, size: key.size);
    _desired = {};
    _queue.clear();
    for (final key in _entries.keys.toList()) {
      _remove(key);
    }
    _completeIdle();
  }

  /// Called only when layout quality changes, never for an ordinary rebuild.
  void invalidateResolution() {
    // Session generations protect document replacement. Resolution validity is
    // the exact desired key: an unchanged quantized size can reuse in-flight work.
    _queue.clear();
    _desired = {};
    _resolutionChanged = true;
  }

  void demand(Iterable<EditorRenderKey> keys) {
    if (_disposed) return;
    final accepted = <EditorRenderKey>{};
    var requestedBytes = 0;
    for (final key in keys) {
      if (key.sessionId != _sessionId ||
          key.bytes > maximumActiveBytes ||
          accepted.length >= maximumEntries ||
          requestedBytes + key.bytes > maximumBytes) {
        continue;
      }
      if (accepted.add(key)) requestedBytes += key.bytes;
    }
    final changed = !setEquals(_desired, accepted);
    _desired = accepted;
    _queue.removeWhere((key) => !accepted.contains(key));
    // Zoom retains at most one temporary older raster per demanded page. Old
    // resolutions disappear once their replacement arrives or the page leaves.
    for (final key in _entries.keys.toList()) {
      if (!accepted.contains(key) &&
          !accepted.any((k) => k.pageIndex == key.pageIndex)) {
        // Keep recent exact pages while scrolling; obsolete resolutions do not accumulate.
        if (_resolutionChanged) _remove(key);
      }
    }
    _resolutionChanged = false;
    for (final key in accepted) {
      if (!_entries.containsKey(key) &&
          !_active.contains(key) &&
          !_queue.contains(key)) {
        _queue.add(key);
      }
    }
    _drain();
    if (changed) notifyListeners();
  }

  EditorRasterState state(EditorRenderKey key) {
    final exact = _entries.remove(key);
    if (exact != null) {
      _entries[key] = exact;
      return exact;
    }
    final fallback = _entries.entries
        .where(
          (entry) =>
              entry.key.sessionId == key.sessionId &&
              entry.key.pageIndex == key.pageIndex &&
              entry.value.image != null,
        )
        .lastOrNull;
    return EditorRasterState(
      image: fallback?.value.image,
      loading: _desired.contains(key) || _desired.isEmpty,
      error: _desired.contains(key) || _desired.isEmpty
          ? null
          : 'Zoom in to view this page.',
    );
  }

  void retry(EditorRenderKey key) {
    _remove(key);
    if (_desired.contains(key) &&
        !_active.contains(key) &&
        !_queue.contains(key)) {
      _queue.add(key);
    }
    _drain();
    notifyListeners();
  }

  void _drain() {
    while (!_disposed &&
        _render != null &&
        _active.length < maximumConcurrent &&
        _queue.isNotEmpty) {
      final key = _queue.removeFirst();
      if (activeBytes + key.bytes > maximumActiveBytes) {
        _queue.addFirst(key);
        break;
      }
      _active.add(key);
      unawaited(_load(key, _generation, _render!));
    }
    _completeIdle();
  }

  Future<void> _load(
    EditorRenderKey key,
    int generation,
    Future<EditorRaster> Function(EditorRenderKey) render,
  ) async {
    ui.Image? image;
    String? error;
    try {
      final raster = await render(key);
      if (_disposed || generation != _generation || !_desired.contains(key)) {
        return;
      }
      if (raster.width != key.size.width || raster.height != key.size.height) {
        throw StateError('Unexpected raster dimensions');
      }
      image = await decode(raster.png);
      if (image.width != key.size.width || image.height != key.size.height) {
        throw StateError('Unexpected decoded dimensions');
      }
    } on ApplicationError catch (problem) {
      error = problem.message;
    } on Object {
      error = 'This page could not be rendered.';
    } finally {
      if (_disposed || generation != _generation || !_desired.contains(key)) {
        image?.dispose();
      } else {
        if (error != null) {
          image?.dispose();
          image = null;
        }
        for (final previous
            in _entries.keys
                .where((k) => k.pageIndex == key.pageIndex)
                .toList()) {
          _remove(previous);
        }
        while (_entries.isNotEmpty &&
            (_entries.length >= maximumEntries ||
                _bytes + (image == null ? 0 : key.bytes) > maximumBytes)) {
          final victim = _entries.keys.firstWhere(
            (k) => !_desired.contains(k),
            orElse: () => _entries.keys.first,
          );
          _remove(victim);
        }
        _entries[key] = EditorRasterState(
          image: image,
          error: error,
          exact: true,
          loading: false,
        );
        if (image != null) _bytes += key.bytes;
        notifyListeners();
      }
      _active.remove(key);
      // An invalidated old request may have prevented enqueueing the new key.
      for (final wanted in _desired) {
        if (!_entries.containsKey(wanted) &&
            !_active.contains(wanted) &&
            !_queue.contains(wanted)) {
          _queue.add(wanted);
        }
      }
      _drain();
    }
  }

  void _remove(EditorRenderKey key) {
    final state = _entries.remove(key);
    final image = state?.image;
    if (image != null) {
      _bytes -= key.bytes;
      // RawImage may still be painted in this frame. Retire after it is replaced.
      SchedulerBinding.instance.addPostFrameCallback((_) => image.dispose());
      SchedulerBinding.instance.scheduleFrame();
    }
  }

  void _completeIdle() {
    if (_active.isNotEmpty || _queue.isNotEmpty) return;
    for (final waiter in _idleWaiters) {
      waiter.complete();
    }
    _idleWaiters.clear();
  }

  @override
  void dispose() {
    _disposed = true;
    _generation++;
    _desired = {};
    _queue.clear();
    for (final key in _entries.keys.toList()) {
      _remove(key);
    }
    _completeIdle();
    super.dispose();
  }

  static Future<ui.Image> decodePng(Uint8List bytes) async {
    final codec = await ui.instantiateImageCodec(bytes);
    try {
      return (await codec.getNextFrame()).image;
    } finally {
      codec.dispose();
    }
  }
}
