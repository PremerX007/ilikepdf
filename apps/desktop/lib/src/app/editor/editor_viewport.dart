import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';

import 'editor_interaction.dart';
import 'editor_object_overlay.dart';
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
    this.edits,
    super.key,
  });
  final EditorSession session;
  final EditorRenderCache cache;
  final ValueChanged<EditorHit?>? onHit;
  final EditorEdits? edits;
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
  final _focus = FocusNode(debugLabel: 'Internal editor objects');
  EditorInteraction? _interaction;

  void _bindInteraction() {
    _interaction?.removeListener(_edited);
    _interaction?.dispose();
    _interaction = widget.edits == null
        ? null
        : EditorInteraction(widget.edits!);
    assert(
      _interaction == null ||
          _interaction!.snapshot.sessionId == widget.session.identity(),
    );
    _interaction?.addListener(_edited);
  }

  void _edited() {
    if (mounted) setState(() {});
  }

  EditorPoint _documentPoint(Offset local) =>
      EditorPoint(x: local.dx + _scroll.x, y: local.dy + _scroll.y);
  void _edit(VoidCallback action) {
    _focus.requestFocus();
    _interaction?.perform(action);
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    // A future text input may own focus inside the editor without these shortcuts.
    if (_focus != FocusManager.instance.primaryFocus ||
        event is! KeyDownEvent ||
        _interaction == null) {
      return KeyEventResult.ignored;
    }
    final edits = _interaction!.edits;
    final control = HardwareKeyboard.instance.isControlPressed;
    final shift = HardwareKeyboard.instance.isShiftPressed;
    if (control && event.logicalKey == LogicalKeyboardKey.keyZ) {
      _edit(() => shift ? edits.redo() : edits.undo());
    } else if (control && event.logicalKey == LogicalKeyboardKey.keyY) {
      _edit(edits.redo);
    } else if (!control && event.logicalKey == LogicalKeyboardKey.delete) {
      _edit(edits.deleteSelected);
    } else if (event.logicalKey == LogicalKeyboardKey.escape) {
      _interaction!.cancel();
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  EditorPoint get _scroll =>
      EditorPoint(x: _horizontal.layoutOffset, y: _vertical.layoutOffset);

  @override
  void initState() {
    super.initState();
    widget.cache.bind(widget.session);
    _bindInteraction();
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
    if (oldWidget.edits != widget.edits ||
        oldWidget.session.identity() != widget.session.identity()) {
      _bindInteraction();
    }
  }

  void _scrolled() {
    _interaction?.cancel(notify: false);
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
    _interaction?.cancel(notify: false);
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
    _interaction?.removeListener(_edited);
    _interaction?.dispose();
    _focus.dispose();
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
                child: ListView(
                  scrollDirection: Axis.horizontal,
                  children: [
                    ConstrainedBox(
                      constraints: BoxConstraints(minWidth: size.width - 32),
                      child: IntrinsicWidth(
                        child: Row(
                          children: [
                            const Icon(Icons.visibility_outlined, size: 18),
                            const SizedBox(width: 8),
                            if (_interaction == null)
                              const Text('Read-only viewport')
                            else ...[
                              TextButton(
                                key: const ValueKey('editor-add-object'),
                                onPressed: () {
                                  final hit = layout.binding.hitTest(
                                    point: EditorPoint(
                                      x: size.width / 2,
                                      y: size.height / 2,
                                    ),
                                    scroll: _scroll,
                                  );
                                  final page =
                                      hit?.pageIndex ??
                                      (visible.isEmpty ? 0 : visible.first);
                                  _edit(
                                    () => _interaction!.edits.addPrototype(
                                      pageIndex: page,
                                    ),
                                  );
                                },
                                child: const Text('Add test object'),
                              ),
                              IconButton(
                                key: const ValueKey('editor-undo'),
                                tooltip: 'Undo (Ctrl+Z)',
                                onPressed: _interaction!.snapshot.undoCount > 0
                                    ? () => _edit(_interaction!.edits.undo)
                                    : null,
                                icon: const Icon(Icons.undo),
                              ),
                              IconButton(
                                key: const ValueKey('editor-redo'),
                                tooltip: 'Redo (Ctrl+Y)',
                                onPressed: _interaction!.snapshot.redoCount > 0
                                    ? () => _edit(_interaction!.edits.redo)
                                    : null,
                                icon: const Icon(Icons.redo),
                              ),
                              IconButton(
                                key: const ValueKey('editor-delete-object'),
                                tooltip: 'Delete object',
                                onPressed:
                                    _interaction!.snapshot.selected != null
                                    ? () => _edit(
                                        _interaction!.edits.deleteSelected,
                                      )
                                    : null,
                                icon: const Icon(Icons.delete_outline),
                              ),
                            ],
                            const Spacer(),
                            if (_interaction?.error != null)
                              Tooltip(
                                message: _interaction!.error!,
                                child: const Icon(
                                  Icons.error_outline,
                                  size: 18,
                                ),
                              ),
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
                  ],
                ),
              ),
            ),
          ),
          Expanded(
            child: Focus(
              focusNode: _focus,
              onKeyEvent: _key,
              onFocusChange: (active) {
                if (!active) _interaction?.cancel();
              },
              child: Listener(
                key: const ValueKey('editor-workspace'),
                behavior: HitTestBehavior.opaque,
                onPointerDown: (event) {
                  widget.onHit?.call(
                    layout.binding.hitTest(
                      point: EditorPoint(
                        x: event.localPosition.dx,
                        y: event.localPosition.dy,
                      ),
                      scroll: _scroll,
                    ),
                  );
                  if (event.buttons == kPrimaryButton && _interaction != null) {
                    _focus.requestFocus();
                    _interaction!.begin(
                      layout.binding,
                      _documentPoint(event.localPosition),
                      event.pointer,
                    );
                  }
                },
                onPointerMove: (event) => _interaction?.update(
                  _documentPoint(event.localPosition),
                  event.pointer,
                ),
                onPointerUp: (event) => _interaction?.finish(
                  _documentPoint(event.localPosition),
                  event.pointer,
                ),
                onPointerCancel: (_) => _interaction?.cancel(),
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
            child: Stack(
              clipBehavior: Clip.none,
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
                if (_interaction != null)
                  EditorObjectOverlay(
                    page: page,
                    objects: _objects(page, layout),
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

  List<EditorObjectDisplay> _objects(
    EditorPageLayout page,
    EditorDocumentLayout layout,
  ) {
    final interaction = _interaction!;
    final objects = interaction.edits.projectPage(
      layout: layout.binding,
      pageIndex: page.pageIndex,
    );
    if (interaction.previewPage != page.pageIndex ||
        interaction.preview == null) {
      return objects;
    }
    return objects
        .map(
          (object) => object.id == interaction.preview!.id
              ? interaction.preview!
              : object,
        )
        .toList();
  }
}
