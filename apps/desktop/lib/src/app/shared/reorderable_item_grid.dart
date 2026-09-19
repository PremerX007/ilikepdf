import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;

typedef OrderedItemBuilder<T> = Widget Function(
  BuildContext context,
  T item,
  int index,
  Widget reorderHandle,
);

class ReorderableItemGrid<T> extends StatelessWidget {
  const ReorderableItemGrid({
    required this.items,
    required this.itemBuilder,
    required this.onReorder,
    this.enabled = true,
    this.minimumItemWidth = 172,
    this.itemHeight = 218,
    this.spacing = 12,
    super.key,
  });

  final List<T> items;
  final OrderedItemBuilder<T> itemBuilder;
  final void Function(int oldIndex, int newIndex) onReorder;
  final bool enabled;
  final double minimumItemWidth;
  final double itemHeight;
  final double spacing;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = math.max(
          1,
          ((constraints.maxWidth + spacing) / (minimumItemWidth + spacing))
              .floor(),
        );
        final width =
            (constraints.maxWidth - spacing * (columns - 1)) / columns;
        return Wrap(
          key: const ValueKey('reorderable-item-grid'),
          spacing: spacing,
          runSpacing: spacing,
          children: List.generate(items.length, (index) {
            return SizedBox(
              width: width,
              height: itemHeight,
              child: _ReorderTarget<T>(
                index: index,
                enabled: enabled,
                onAccept: (oldIndex) => onReorder(oldIndex, index),
                childBuilder: (isTargeted) => itemBuilder(
                  context,
                  items[index],
                  index,
                  _ReorderHandle(
                    index: index,
                    enabled: enabled,
                    previewWidth: width,
                    previewHeight: itemHeight,
                  ),
                ),
              ),
            );
          }),
        );
      },
    );
  }
}

/// Lazily builds a reorderable page grid so large workspaces only instantiate
/// cards near the viewport. Thumbnail ownership remains with the calling tool.
class LazyReorderableItemGrid<T> extends StatefulWidget {
  const LazyReorderableItemGrid({
    required this.items,
    required this.itemBuilder,
    required this.onReorder,
    this.enabled = true,
    this.maximumItemWidth = 210,
    this.itemHeight = 286,
    this.spacing = 12,
    this.padding = const EdgeInsets.all(16),
    super.key,
  });

  final List<T> items;
  final OrderedItemBuilder<T> itemBuilder;
  final void Function(int oldIndex, int newIndex) onReorder;
  final bool enabled;
  final double maximumItemWidth;
  final double itemHeight;
  final double spacing;
  final EdgeInsetsGeometry padding;

  @override
  State<LazyReorderableItemGrid<T>> createState() =>
      _LazyReorderableItemGridState<T>();
}

class _LazyReorderableItemGridState<T>
    extends State<LazyReorderableItemGrid<T>> {
  static const _autoScrollEdge = 72.0;
  static const _autoScrollStep = 36.0;
  final ScrollController _scrollController = ScrollController();

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return GridView.builder(
      key: const ValueKey('lazy-reorderable-item-grid'),
      controller: _scrollController,
      padding: widget.padding,
      scrollCacheExtent: ScrollCacheExtent.pixels(widget.itemHeight),
      gridDelegate: SliverGridDelegateWithMaxCrossAxisExtent(
        maxCrossAxisExtent: widget.maximumItemWidth,
        mainAxisExtent: widget.itemHeight,
        mainAxisSpacing: widget.spacing,
        crossAxisSpacing: widget.spacing,
      ),
      itemCount: widget.items.length,
      itemBuilder: (context, index) => _ReorderTarget<T>(
        key: ValueKey('lazy-reorder-target-$index'),
        index: index,
        enabled: widget.enabled,
        onAccept: (oldIndex) => widget.onReorder(oldIndex, index),
        onDragMove: _autoScrollForDrag,
        childBuilder: (_) => widget.itemBuilder(
          context,
          widget.items[index],
          index,
          _ReorderHandle(
            index: index,
            enabled: widget.enabled,
            previewWidth: widget.maximumItemWidth,
            previewHeight: widget.itemHeight,
          ),
        ),
      ),
    );
  }

  void _autoScrollForDrag(Offset globalPosition) {
    if (!_scrollController.hasClients) return;
    final renderObject = context.findRenderObject();
    if (renderObject is! RenderBox || !renderObject.hasSize) return;
    final localPosition = renderObject.globalToLocal(globalPosition);
    final distance = localPosition.dy < _autoScrollEdge
        ? -_autoScrollStep
        : localPosition.dy > renderObject.size.height - _autoScrollEdge
        ? _autoScrollStep
        : 0.0;
    if (distance == 0) return;
    final position = _scrollController.position;
    final target = (position.pixels + distance)
        .clamp(position.minScrollExtent, position.maxScrollExtent)
        .toDouble();
    if (target != position.pixels) _scrollController.jumpTo(target);
  }
}

class _ReorderTarget<T> extends StatelessWidget {
  const _ReorderTarget({
    required this.index,
    required this.enabled,
    required this.onAccept,
    required this.childBuilder,
    this.onDragMove,
    super.key,
  });

  final int index;
  final bool enabled;
  final ValueChanged<int> onAccept;
  final Widget Function(bool isTargeted) childBuilder;
  final ValueChanged<Offset>? onDragMove;

  @override
  Widget build(BuildContext context) {
    return DragTarget<int>(
      key: ValueKey('reorder-target-$index'),
      onWillAcceptWithDetails: (details) => enabled && details.data != index,
      onMove: enabled ? (details) => onDragMove?.call(details.offset) : null,
      onAcceptWithDetails: (details) => onAccept(details.data),
      builder: (context, candidateData, rejectedData) => AnimatedScale(
        duration: const Duration(milliseconds: 100),
        scale: candidateData.isEmpty ? 1 : 0.97,
        child: childBuilder(candidateData.isNotEmpty),
      ),
    );
  }
}

class _ReorderHandle extends StatelessWidget {
  const _ReorderHandle({
    required this.index,
    required this.enabled,
    required this.previewWidth,
    required this.previewHeight,
  });

  final int index;
  final bool enabled;
  final double previewWidth;
  final double previewHeight;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final handle = Tooltip(
      message: 'Drag to reorder',
      child: Icon(
        Icons.drag_indicator_rounded,
        size: 20,
        color: enabled ? colors.onSurfaceVariant : colors.outline,
      ),
    );
    if (!enabled) {
      return handle;
    }
    return Draggable<int>(
      key: ValueKey('reorder-item-$index'),
      data: index,
      feedback: Material(
        color: Colors.transparent,
        child: Opacity(
          opacity: 0.86,
          child: Container(
            width: previewWidth,
            height: previewHeight,
            decoration: BoxDecoration(
              color: colors.surface,
              borderRadius: BorderRadius.circular(16),
              border: Border.all(color: colors.primary, width: 2),
              boxShadow: const [
                BoxShadow(
                  color: Color(0x33000000),
                  blurRadius: 20,
                  offset: Offset(0, 10),
                ),
              ],
            ),
            child: const Center(child: Icon(Icons.drag_indicator_rounded)),
          ),
        ),
      ),
      childWhenDragging: Opacity(opacity: 0.35, child: handle),
      child: handle,
    );
  }
}
