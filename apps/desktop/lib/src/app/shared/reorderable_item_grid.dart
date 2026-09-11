import 'dart:math' as math;

import 'package:flutter/material.dart';

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

class _ReorderTarget<T> extends StatelessWidget {
  const _ReorderTarget({
    required this.index,
    required this.enabled,
    required this.onAccept,
    required this.childBuilder,
  });

  final int index;
  final bool enabled;
  final ValueChanged<int> onAccept;
  final Widget Function(bool isTargeted) childBuilder;

  @override
  Widget build(BuildContext context) {
    return DragTarget<int>(
      key: ValueKey('reorder-target-$index'),
      onWillAcceptWithDetails: (details) => enabled && details.data != index,
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
