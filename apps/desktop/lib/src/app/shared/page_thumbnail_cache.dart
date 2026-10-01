import 'dart:async';
import 'dart:collection';
import 'dart:io';

import 'package:flutter/material.dart';

/// Viewport-demanded previews, independent of page grouping or workflow state.
/// Owns only the temporary images returned by [render], never source PDFs.
class PageThumbnailCache {
  PageThumbnailCache({required this.render, this.previousSessionIdle});

  static const maximumCached = 32;
  static const maximumConcurrent = 2;
  final Future<String> Function(int pageIndex) render;
  final Future<void>? previousSessionIdle;
  final _disposedAndIdle = Completer<void>();
  final _cached = <int, String?>{};
  final _listeners = <int, Set<ValueNotifier<PageThumbnailState>>>{};
  final _queue = Queue<int>();
  final _active = <int>{};
  bool _disposed = false;

  int get cachedCount => _cached.length;
  int get activeCount => _active.length;
  int get queuedCount => _queue.length;
  Future<void> get disposedAndIdle async {
    await previousSessionIdle;
    await _disposedAndIdle.future;
  }

  ValueNotifier<PageThumbnailState> acquire(int pageIndex) {
    assert(!_disposed);
    final cached = _cached.containsKey(pageIndex);
    final path = _cached.remove(pageIndex);
    if (cached) _cached[pageIndex] = path;
    final listener = ValueNotifier(
      PageThumbnailState(path: path, loading: !cached),
    );
    (_listeners[pageIndex] ??= {}).add(listener);
    if (!cached &&
        !_active.contains(pageIndex) &&
        !_queue.contains(pageIndex)) {
      _queue.add(pageIndex);
    }
    _drain();
    return listener;
  }

  void release(int pageIndex, ValueNotifier<PageThumbnailState> listener) {
    final listeners = _listeners[pageIndex];
    listeners?.remove(listener);
    if (listeners?.isEmpty ?? false) {
      _listeners.remove(pageIndex);
      _queue.remove(pageIndex);
    }
    listener.dispose();
  }

  void _drain() {
    while (!_disposed &&
        _active.length < maximumConcurrent &&
        _queue.isNotEmpty) {
      final page = _queue.removeFirst();
      _active.add(page);
      _load(page);
    }
  }

  Future<void> _load(int page) async {
    String? path;
    try {
      // Replacing/clearing a source cannot overlap new native work with the
      // previous session's two uncancellable in-flight renders.
      if (previousSessionIdle != null) await previousSessionIdle;
      if (_disposed || !_listeners.containsKey(page)) {
        _active.remove(page);
        _completeDisposal();
        _drain();
        return;
      }
      if (!_disposed) path = await render(page);
    } on Object {
      // Failure is a bounded cache entry, so scrolling cannot create a retry loop.
    }
    if (_disposed) {
      if (path != null) _delete(path);
    } else {
      while (_cached.length >= maximumCached) {
        final victim = _cached.keys.firstWhere(
          (key) => !_listeners.containsKey(key),
          orElse: () => _cached.keys.first,
        );
        final removed = _cached.remove(victim);
        _notify(victim, const PageThumbnailState(loading: false));
        if (removed != null) _delete(removed);
      }
      _cached[page] = path;
      _notify(page, PageThumbnailState(path: path, loading: false));
    }
    _active.remove(page);
    _completeDisposal();
    _drain();
  }

  void _notify(int page, PageThumbnailState state) {
    for (final listener
        in _listeners[page] ?? <ValueNotifier<PageThumbnailState>>{}) {
      listener.value = state;
    }
  }

  void dispose() {
    _disposed = true;
    _queue.clear();
    for (final path in _cached.values.whereType<String>()) {
      _delete(path);
    }
    _cached.clear();
    _completeDisposal();
    // Tile leases dispose their own notifiers. In-flight results are discarded.
  }

  void _completeDisposal() {
    if (_disposed && _active.isEmpty && !_disposedAndIdle.isCompleted) {
      _disposedAndIdle.complete();
    }
  }

  void _delete(String path) {
    FileImage(File(path)).evict().ignore();
    File(path).delete().ignore();
  }
}

class PageThumbnailState {
  const PageThumbnailState({this.path, required this.loading});

  final String? path;
  final bool loading;
}

class LazyPageThumbnail extends StatefulWidget {
  const LazyPageThumbnail({
    required this.cache,
    required this.pageIndex,
    super.key,
  });

  final PageThumbnailCache cache;
  final int pageIndex;

  @override
  State<LazyPageThumbnail> createState() => _LazyPageThumbnailState();
}

class _LazyPageThumbnailState extends State<LazyPageThumbnail> {
  late ValueNotifier<PageThumbnailState> _state;

  @override
  void initState() {
    super.initState();
    _state = widget.cache.acquire(widget.pageIndex);
  }

  @override
  void didUpdateWidget(LazyPageThumbnail oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.cache != oldWidget.cache ||
        widget.pageIndex != oldWidget.pageIndex) {
      oldWidget.cache.release(oldWidget.pageIndex, _state);
      _state = widget.cache.acquire(widget.pageIndex);
    }
  }

  @override
  void dispose() {
    widget.cache.release(widget.pageIndex, _state);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ValueListenableBuilder(
    valueListenable: _state,
    builder: (context, state, _) {
      if (state.loading) {
        return const Center(
          child: SizedBox.square(
            dimension: 20,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
        );
      }
      if (state.path case final path?) {
        return Image.file(
          File(path),
          fit: BoxFit.contain,
          key: ValueKey('page-thumbnail-${widget.pageIndex}'),
          errorBuilder: (_, _, _) => const _UnavailableThumbnail(),
        );
      }
      return const _UnavailableThumbnail();
    },
  );
}

class _UnavailableThumbnail extends StatelessWidget {
  const _UnavailableThumbnail();

  @override
  Widget build(BuildContext context) => const Center(
    child: Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(Icons.broken_image_outlined),
        SizedBox(height: 6),
        Text('Preview unavailable', textAlign: TextAlign.center),
      ],
    ),
  );
}
