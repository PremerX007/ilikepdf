import 'dart:math' as math;

import 'package:flutter/gestures.dart' show kPrimaryButton;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;

typedef OrderedItemBuilder<T> = Widget Function(
  BuildContext context,
  T item,
  int index,
  Widget reorderHandle,
);

/// Wrap only the non-interactive surface. Place action controls above it as
/// siblings so a pointer beginning on a control cannot enter the drag recognizer.
typedef ReorderableDragBuilder = Widget Function({
  required Widget child,
  required Widget feedback,
});

typedef LazyOrderedItemBuilder<T> = Widget Function(
  BuildContext context,
  T item,
  int index,
  ReorderableDragBuilder dragSurface,
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
  final LazyOrderedItemBuilder<T> itemBuilder;
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
      itemBuilder: (context, index) => LayoutBuilder(
        builder: (context, constraints) => _ReorderTarget<T>(
          key: ValueKey('lazy-reorder-target-$index'),
          index: index,
          enabled: widget.enabled,
          showInsertionMarker: true,
          onAccept: (oldIndex) => widget.onReorder(oldIndex, index),
          childBuilder: (_) => widget.itemBuilder(
            context,
            widget.items[index],
            index,
            ({required child, required feedback}) => _ReorderSurface(
              index: index,
              enabled: widget.enabled,
              size: constraints.biggest,
              feedback: feedback,
              onDragMove: _autoScrollForDrag,
              child: child,
            ),
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
    this.showInsertionMarker = false,
    super.key,
  });

  final int index;
  final bool enabled;
  final ValueChanged<int> onAccept;
  final Widget Function(bool isTargeted) childBuilder;
  final bool showInsertionMarker;

  @override
  Widget build(BuildContext context) {
    return DragTarget<int>(
      key: ValueKey('reorder-target-$index'),
      onWillAcceptWithDetails: (details) => enabled && details.data != index,
      onAcceptWithDetails: (details) => onAccept(details.data),
      builder: (context, candidateData, rejectedData) {
        final targeted = candidateData.isNotEmpty;
        if (showInsertionMarker) {
          return Stack(
            fit: StackFit.expand,
            clipBehavior: Clip.none,
            children: [
              childBuilder(targeted),
              if (targeted)
                Positioned(
                  key: ValueKey('reorder-insertion-$index'),
                  // Existing move-to-index semantics insert before a target
                  // when moving backwards, and after it when moving forwards.
                  left: candidateData.first! > index ? -6 : null,
                  right: candidateData.first! < index ? -6 : null,
                  top: 4,
                  bottom: 4,
                  child: IgnorePointer(
                    child: Container(
                      width: 3,
                      decoration: BoxDecoration(
                        color: Theme.of(context).colorScheme.primary,
                        borderRadius: BorderRadius.circular(2),
                      ),
                    ),
                  ),
                ),
            ],
          );
        }
        return AnimatedScale(
          duration: const Duration(milliseconds: 100),
          scale: targeted ? 0.97 : 1,
          child: childBuilder(targeted),
        );
      },
    );
  }
}

class _ReorderSurface extends StatelessWidget {
  const _ReorderSurface({
    required this.index,
    required this.enabled,
    required this.size,
    required this.child,
    required this.feedback,
    required this.onDragMove,
  });

  final int index;
  final bool enabled;
  final Size size;
  final Widget child;
  final Widget feedback;
  final ValueChanged<Offset> onDragMove;

  @override
  Widget build(BuildContext context) {
    if (!enabled) return child;
    return MouseRegion(
      cursor: SystemMouseCursors.grab,
      child: Draggable<int>(
        data: index,
        maxSimultaneousDrags: 1,
        allowedButtonsFilter: (buttons) => buttons == kPrimaryButton,
        hitTestBehavior: HitTestBehavior.opaque,
        onDragUpdate: (details) => onDragMove(details.globalPosition),
        feedback: Material(
          key: const ValueKey('reorder-card-proxy'),
          color: Colors.transparent,
          elevation: 6,
          borderRadius: BorderRadius.circular(14),
          child: SizedBox.fromSize(size: size, child: feedback),
        ),
        childWhenDragging: Opacity(opacity: 0.35, child: child),
        // A tap recognizer keeps the gesture arena open until movement exceeds
        // Flutter's drag threshold. Without it, an otherwise uncontested
        // Draggable can win on pointer-down, even for an ordinary mouse click.
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () {},
          child: child,
        ),
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
