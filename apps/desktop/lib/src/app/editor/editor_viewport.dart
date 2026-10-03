import 'package:flutter/material.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';

import 'editor_render_cache.dart';
import 'editor_scroll_controller.dart';
import 'editor_zoom.dart';

/// The document surface positions raster and future overlays from core's exact
/// rectangles. No PDF coordinate conversion or page geometry rules live here.
class EditorViewport extends StatefulWidget {
  const EditorViewport({
    required this.session,
    required this.cache,
    this.onHit,
    super.key,
  });
  final EditorSession session;
  final EditorRenderCache cache;
  final ValueChanged<EditorHit?>? onHit;
  @override
  State<EditorViewport> createState() => _EditorViewportState();
}

class _EditorViewportState extends State<EditorViewport> {
  final _vertical = EditorScrollController();
  final _horizontal = EditorScrollController();
  EditorDocumentLayout? _layout;
  Size _viewportSize = Size.zero;
  EditorViewportZoom _zoom = const EditorViewportZoom.initial();
  double _density = 1.0;
  bool _demandScheduled = false;

  EditorPoint get _scroll =>
      EditorPoint(x: _horizontal.layoutOffset, y: _vertical.layoutOffset);

  @override
  void initState() {
    super.initState();
    widget.cache.bind(widget.session);
    _vertical.addListener(_scrolled);
    _horizontal.addListener(_scrolled);
  }

  @override
  void didUpdateWidget(EditorViewport oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.session.identity() != widget.session.identity()) {
      widget.cache.bind(widget.session);
      _layout = null;
      _zoom = const EditorViewportZoom.initial();
      _horizontal.stageOffset(0);
      _vertical.stageOffset(0);
    }
  }

  void _scrolled() {
    if (mounted) setState(() {});
    _scheduleDemand();
  }

  void _scheduleDemand() {
    if (_demandScheduled) return;
    _demandScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _demandScheduled = false;
      if (!mounted || _layout == null) return;
      final layout = _layout!;
      final pages = layout.binding.visiblePages(
        top: _scroll.y,
        height: _viewportSize.height,
        overscan: 120,
      );
      widget.cache.demand(
        pages.map(
          (index) => EditorRenderKey(
            widget.session.identity(),
            index,
            layout.binding.renderSize(pageIndex: index, density: _density),
          ),
        ),
      );
    });
  }

  void _relayout(Size size) {
    final old = _layout;
    final next = widget.session.layout(
      workspaceWidth: size.width,
      scale: _zoom.scale,
      fitWidth: _zoom.fitsWidth,
    );
    if (old != null && (old.scale != next.scale || size != _viewportSize)) {
      final anchor = next.binding.anchoredScroll(
        previous: old.binding,
        scroll: _scroll,
        viewport: EditorPoint(x: size.width, y: size.height),
      );
      _horizontal.stageOffset(anchor.x);
      _vertical.stageOffset(anchor.y);
    }
    _layout = next;
    _zoom = _zoom.resolved(next.scale);
    _viewportSize = size;
    widget.cache.invalidateResolution();
    _scheduleDemand();
  }

  void _changeZoom({bool? increase, bool fit = false, bool reset = false}) {
    setState(() {
      if (fit) _zoom = _zoom.fitWidth();
      if (reset) _zoom = const EditorViewportZoom.actualSize();
      if (increase != null) {
        _zoom = _zoom.step(increase);
      }
      _relayout(_viewportSize);
    });
  }

  @override
  void dispose() {
    _vertical.dispose();
    _horizontal.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      const toolbarHeight = 60.0;
      final size = Size(
        constraints.maxWidth,
        constraints.maxHeight - toolbarHeight,
      );
      if (size.width <= 0 || size.height <= 0) return const SizedBox.shrink();
      final density = MediaQuery.devicePixelRatioOf(context).clamp(0.5, 8.0);
      if (_layout == null || size != _viewportSize) {
        _density = density;
        _relayout(size);
      } else if (density != _density) {
        _density = density;
        widget.cache.invalidateResolution();
      }
      final layout = _layout!;
      final visible = layout.binding.visiblePages(
        top: _scroll.y,
        height: size.height,
        overscan: 120,
      );
      _scheduleDemand();
      return Column(
        children: [
          SizedBox(
            height: toolbarHeight,
            child: Material(
              color: Theme.of(context).colorScheme.surface,
              child: Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 6,
                ),
                child: Row(
                  children: [
                    const Icon(Icons.visibility_outlined, size: 18),
                    const SizedBox(width: 8),
                    const Text('Read-only viewport'),
                    const Spacer(),
                    IconButton(
                      key: const ValueKey('editor-zoom-out'),
                      tooltip: 'Zoom out',
                      onPressed: _zoom.canDecrease
                          ? () => _changeZoom(increase: false)
                          : null,
                      icon: const Icon(Icons.remove),
                    ),
                    SizedBox(
                      width: 64,
                      child: Text(
                        '${(_zoom.scale * 100).round()}%',
                        key: const ValueKey('editor-zoom-value'),
                        textAlign: TextAlign.center,
                      ),
                    ),
                    IconButton(
                      key: const ValueKey('editor-zoom-in'),
                      tooltip: 'Zoom in',
                      onPressed: _zoom.canIncrease
                          ? () => _changeZoom(increase: true)
                          : null,
                      icon: const Icon(Icons.add),
                    ),
                    TextButton(
                      key: const ValueKey('editor-reset-zoom'),
                      onPressed: () => _changeZoom(reset: true),
                      child: const Text('100%'),
                    ),
                    TextButton(
                      key: const ValueKey('editor-fit-width'),
                      onPressed: () => _changeZoom(fit: true),
                      child: const Text('Fit width'),
                    ),
                  ],
                ),
              ),
            ),
          ),
          Expanded(
            child: Listener(
              key: const ValueKey('editor-workspace'),
              behavior: HitTestBehavior.opaque,
              onPointerDown: (event) => widget.onHit?.call(
                layout.binding.hitTest(
                  point: EditorPoint(
                    x: event.localPosition.dx,
                    y: event.localPosition.dy,
                  ),
                  scroll: _scroll,
                ),
              ),
              child: ColoredBox(
                color: Theme.of(context).colorScheme.surfaceContainerHighest,
                child: Scrollbar(
                  controller: _vertical,
                  thumbVisibility: true,
                  notificationPredicate: (notification) =>
                      notification.metrics.axis == Axis.vertical,
                  child: Scrollbar(
                    controller: _horizontal,
                    thumbVisibility: layout.width > size.width,
                    notificationPredicate: (notification) =>
                        notification.metrics.axis == Axis.horizontal,
                    child: SingleChildScrollView(
                      key: ValueKey(widget.session.identity()),
                      controller: _horizontal,
                      scrollDirection: Axis.horizontal,
                      child: SizedBox(
                        width: layout.width,
                        height: size.height,
                        child: SingleChildScrollView(
                          key: const ValueKey('editor-vertical-scroll'),
                          controller: _vertical,
                          child: SizedBox(
                            width: layout.width,
                            height: layout.height,
                            child: AnimatedBuilder(
                              animation: widget.cache,
                              builder: (context, _) => Stack(
                                children: [
                                  for (final index in visible)
                                    ..._page(
                                      context,
                                      layout.pages[index],
                                      layout,
                                    ),
                                ],
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ],
      );
    },
  );

  List<Widget> _page(
    BuildContext context,
    EditorPageLayout page,
    EditorDocumentLayout layout,
  ) {
    final rect = page.rect;
    final key = EditorRenderKey(
      widget.session.identity(),
      page.pageIndex,
      layout.binding.renderSize(pageIndex: page.pageIndex, density: _density),
    );
    final state = widget.cache.state(key);
    return [
      Positioned(
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        child: RepaintBoundary(
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: Colors.white,
              boxShadow: const [
                BoxShadow(
                  color: Color(0x26000000),
                  blurRadius: 5,
                  offset: Offset(0, 2),
                ),
              ],
            ),
            // Future object overlay belongs in this Stack using page.transform.
            child: Stack(
              fit: StackFit.expand,
              children: [
                if (state.image != null)
                  RawImage(
                    key: ValueKey('editor-raster-${page.pageIndex}'),
                    image: state.image,
                    fit: BoxFit.fill,
                    // Native text AA stays enabled. Near display resolution,
                    // bilinear filtering handles fractional placement without mipmaps.
                    filterQuality: FilterQuality.low,
                  ),
                if (state.loading && state.image == null)
                  const Center(
                    child: SizedBox.square(
                      dimension: 24,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ),
                if (state.error != null)
                  Center(
                    child: Padding(
                      padding: const EdgeInsets.all(12),
                      child: SingleChildScrollView(
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            const Icon(Icons.broken_image_outlined),
                            const SizedBox(height: 8),
                            Text(state.error!, textAlign: TextAlign.center),
                            TextButton(
                              onPressed: () => widget.cache.retry(key),
                              child: const Text('Retry page'),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                IgnorePointer(
                  child: DecoratedBox(
                    key: ValueKey('editor-page-${page.pageIndex}'),
                    decoration: BoxDecoration(
                      border: Border.all(color: const Color(0xFFB8B8B8)),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
      Positioned(
        left: rect.left,
        top: rect.top + rect.height + 3,
        width: rect.width,
        height: 18,
        child: Text(
          'Page ${page.pageIndex + 1}',
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.labelSmall,
        ),
      ),
    ];
  }
}
