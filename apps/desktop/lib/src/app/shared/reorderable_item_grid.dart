import 'dart:math' as math;

import 'package:flutter/foundation.dart' show listEquals;
import 'package:flutter/gestures.dart' show kPrimaryButton;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;

/// Wrap the non-interactive surface; keep action controls above it as siblings.
typedef ReorderableDragBuilder = Widget Function({
  required Widget child,
  required Widget feedback,
});

typedef OrderedItemBuilder<T> = Widget Function(
  BuildContext context,
  T item,
  int index,
  ReorderableDragBuilder dragSurface,
);

class ReorderableItemGrid<T> extends StatefulWidget {
  const ReorderableItemGrid({
    required this.items,
    required this.itemKey,
    required this.itemBuilder,
    required this.onReorder,
    this.enabled = true,
    this.minimumItemWidth = 172,
    this.itemHeight = 218,
    this.spacing = 12,
    super.key,
  });

  final List<T> items;
  final Key Function(T item) itemKey;
  final OrderedItemBuilder<T> itemBuilder;
  final void Function(int oldIndex, int newIndex) onReorder;
  final bool enabled;
  final double minimumItemWidth;
  final double itemHeight;
  final double spacing;

  @override
  State<ReorderableItemGrid<T>> createState() => _ReorderableItemGridState<T>();
}

class _ReorderableItemGridState<T>
    extends _ReorderGridState<T, ReorderableItemGrid<T>> {
  @override
  List<T> get items => widget.items;
  @override
  Key Function(T) get itemKey => widget.itemKey;
  @override
  OrderedItemBuilder<T> get itemBuilder => widget.itemBuilder;
  @override
  bool get enabled => widget.enabled;
  @override
  void Function(int, int) get onReorder => widget.onReorder;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final columns = math.max(
        1,
        ((constraints.maxWidth + widget.spacing) /
                (widget.minimumItemWidth + widget.spacing))
            .floor(),
      );
      geometry = _GridGeometry(
        columns: columns,
        width:
            (constraints.maxWidth - widget.spacing * (columns - 1)) / columns,
        height: widget.itemHeight,
        spacing: widget.spacing,
        direction: Directionality.of(context),
      );
      return buildDragTarget(
        Wrap(
          key: const ValueKey('reorderable-item-grid'),
          spacing: widget.spacing,
          runSpacing: widget.spacing,
          children: List.generate(items.length, (index) => buildItem(index)),
        ),
      );
    },
  );
}

/// Keeps original grid slots during drag. Only near-visible cards are built;
/// small translations preview the pending order without eager thumbnail work.
class LazyReorderableItemGrid<T> extends StatefulWidget {
  const LazyReorderableItemGrid({
    required this.items,
    required this.itemKey,
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
  final Key Function(T item) itemKey;
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
    extends _ReorderGridState<T, LazyReorderableItemGrid<T>> {
  final ScrollController _scrollController = ScrollController();
  EdgeInsets _padding = EdgeInsets.zero;

  @override
  List<T> get items => widget.items;
  @override
  Key Function(T) get itemKey => widget.itemKey;
  @override
  OrderedItemBuilder<T> get itemBuilder => widget.itemBuilder;
  @override
  bool get enabled => widget.enabled;
  @override
  void Function(int, int) get onReorder => widget.onReorder;
  @override
  Offset get contentOffset => Offset(
    -_padding.left,
    (_scrollController.hasClients ? _scrollController.offset : 0) -
        _padding.top,
  );

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final direction = Directionality.of(context);
      _padding = widget.padding.resolve(direction);
      final width = constraints.maxWidth - _padding.horizontal;
      final columns = math.max(
        1,
        (width / (widget.maximumItemWidth + widget.spacing)).ceil(),
      );
      geometry = _GridGeometry(
        columns: columns,
        width: (width - widget.spacing * (columns - 1)) / columns,
        height: widget.itemHeight,
        spacing: widget.spacing,
        direction: direction,
      );
      return buildDragTarget(
        GridView.builder(
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
          itemCount: items.length,
          findChildIndexCallback: (key) {
            final index = items.indexWhere((item) => itemKey(item) == key);
            return index < 0 ? null : index;
          },
          itemBuilder: (context, index) => buildItem(index, lazy: true),
        ),
      );
    },
  );

  @override
  void onDragMove(Offset globalPosition) {
    if (!_validDrag) return;
    if (_scrollController.hasClients) {
      final box = context.findRenderObject()! as RenderBox;
      final local = box.globalToLocal(globalPosition);
      if (local.dx >= 0 && local.dx <= box.size.width) {
        final step = local.dy < 72
            ? -36.0
            : local.dy > box.size.height - 72
            ? 36.0
            : 0.0;
        final position = _scrollController.position;
        final target = (position.pixels + step)
            .clamp(position.minScrollExtent, position.maxScrollExtent)
            .toDouble();
        if (target != position.pixels) _scrollController.jumpTo(target);
      }
    }
    super.onDragMove(globalPosition);
  }
}

/// Preview state is local to the grid. The tool owns the committed order.
abstract class _ReorderGridState<T, W extends StatefulWidget> extends State<W> {
  List<T> get items;
  Key Function(T) get itemKey;
  OrderedItemBuilder<T> get itemBuilder;
  bool get enabled;
  void Function(int, int) get onReorder;
  Offset get contentOffset => Offset.zero;

  final Object _owner = Object();
  late _GridGeometry geometry;
  int? _draggedIndex;
  int? _targetIndex;
  List<Key>? _dragKeys;
  bool _snapToLayout = false;

  bool get _validDrag =>
      enabled &&
      _draggedIndex != null &&
      items.isNotEmpty &&
      _dragKeys?.length == items.length;

  // Validate on tool updates and drop, rather than scanning a large page list
  // for every pointer movement.
  bool get _sameItems =>
      listEquals(_dragKeys, items.map(itemKey).toList(growable: false));

  @override
  void didUpdateWidget(covariant W oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_draggedIndex != null && (!_validDrag || !_sameItems)) {
      _clearDrag(snap: true);
    }
  }

  void _clearDrag({bool snap = false}) {
    _draggedIndex = null;
    _targetIndex = null;
    _dragKeys = null;
    _snapToLayout = snap;
  }

  void onDragMove(Offset globalPosition) {
    _updateTarget(globalPosition);
  }

  void _updateTarget(Offset globalPosition) {
    if (!_validDrag) return;
    final box = context.findRenderObject()! as RenderBox;
    final local = box.globalToLocal(globalPosition);
    if (!(Offset.zero & box.size).contains(local)) return;
    final target = geometry.indexAt(local + contentOffset, items.length);
    if (target != _targetIndex) setState(() => _targetIndex = target);
  }

  Widget buildDragTarget(Widget child) => DragTarget<_GridDrag>(
    onWillAcceptWithDetails: (details) =>
        // Flutter enters the target before calling onDragStarted. Validate the
        // owner here, then validate the saved item set again when dropping.
        identical(details.data.owner, _owner) && enabled,
    onMove: (details) => onDragMove(details.data.pointerAt(details.offset)),
    onLeave: (_) {
      if (_draggedIndex != null) setState(() => _targetIndex = _draggedIndex);
    },
    onAcceptWithDetails: (details) {
      if (!identical(details.data.owner, _owner) ||
          !_validDrag ||
          !_sameItems) {
        return;
      }
      _updateTarget(details.data.pointerAt(details.offset));
      final oldIndex = _draggedIndex!;
      final newIndex = _targetIndex!;
      setState(() => _clearDrag(snap: true));
      if (oldIndex != newIndex) onReorder(oldIndex, newIndex);
    },
    builder: (context, candidates, rejected) => child,
  );

  int _visualIndex(int index) {
    final from = _draggedIndex;
    final to = _targetIndex;
    if (from == null || to == null) return index;
    if (index == from) return to;
    if (from < to && index > from && index <= to) return index - 1;
    if (from > to && index >= to && index < from) return index + 1;
    return index;
  }

  Widget buildItem(int index, {bool lazy = false}) {
    final dragged = _draggedIndex == index;
    final offset =
        geometry.position(_visualIndex(index)) - geometry.position(index);
    return SizedBox(
      key: itemKey(items[index]),
      width: geometry.width,
      height: geometry.height,
      child: Stack(
        clipBehavior: Clip.none,
        fit: StackFit.expand,
        children: [
          IgnorePointer(
            child: SizedBox.expand(
              key: ValueKey('${lazy ? 'lazy-' : ''}reorder-target-$index'),
            ),
          ),
          if (_targetIndex == index)
            DecoratedBox(
              key: const ValueKey('reorder-placeholder'),
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.primary
                    .withValues(alpha: 0.06),
                borderRadius: BorderRadius.circular(14),
                border: Border.all(
                  color: Theme.of(context).colorScheme.outline,
                ),
              ),
            ),
          AnimatedSlide(
            key: const ValueKey('reorder-card-slide'),
            duration: _snapToLayout
                ? Duration.zero
                : const Duration(milliseconds: 160),
            curve: Curves.easeOutCubic,
            offset: Offset(
              offset.dx / geometry.width,
              offset.dy / geometry.height,
            ),
            child: IgnorePointer(
              ignoring: dragged,
              child: Opacity(
                opacity: dragged ? 0 : 1,
                child: itemBuilder(
                  context,
                  items[index],
                  index,
                  ({required child, required feedback}) => _ReorderSurface(
                    data: _GridDrag(_owner),
                    enabled: enabled && _draggedIndex == null,
                    size: Size(geometry.width, geometry.height),
                    onDragStarted: () => setState(() {
                      _draggedIndex = index;
                      _targetIndex = index;
                      _dragKeys = items.map(itemKey).toList(growable: false);
                      _snapToLayout = false;
                    }),
                    onDragEnd: () {
                      if (mounted && _draggedIndex != null) {
                        setState(_clearDrag);
                      }
                    },
                    feedback: feedback,
                    child: child,
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _GridGeometry {
  const _GridGeometry({
    required this.columns,
    required this.width,
    required this.height,
    required this.spacing,
    required this.direction,
  });
  final int columns;
  final double width;
  final double height;
  final double spacing;
  final TextDirection direction;

  Offset position(int index) => Offset(
    (direction == TextDirection.ltr
            ? index % columns
            : columns - 1 - index % columns) *
        (width + spacing),
    (index ~/ columns) * (height + spacing),
  );

  int indexAt(Offset position, int itemCount) {
    var column = (position.dx / (width + spacing)).floor().clamp(
      0,
      columns - 1,
    );
    if (direction == TextDirection.rtl) column = columns - 1 - column;
    final row = math.max(0, (position.dy / (height + spacing)).floor());
    return (row * columns + column).clamp(0, itemCount - 1);
  }
}

class _GridDrag {
  _GridDrag(this.owner);
  final Object owner;
  Offset anchor = Offset.zero;

  // DragTargetDetails.offset is the feedback's top-left, not the pointer.
  Offset pointerAt(Offset feedbackOffset) => feedbackOffset + anchor;
}

class _ReorderSurface extends StatelessWidget {
  const _ReorderSurface({
    required this.data,
    required this.enabled,
    required this.size,
    required this.child,
    required this.feedback,
    required this.onDragStarted,
    required this.onDragEnd,
  });
  final _GridDrag data;
  final bool enabled;
  final Size size;
  final Widget child;
  final Widget feedback;
  final VoidCallback onDragStarted;
  final VoidCallback onDragEnd;

  @override
  Widget build(BuildContext context) => MouseRegion(
    cursor: enabled ? SystemMouseCursors.grab : MouseCursor.defer,
    child: Draggable<_GridDrag>(
      data: data,
      maxSimultaneousDrags: enabled ? 1 : 0,
      allowedButtonsFilter: (buttons) => buttons == kPrimaryButton,
      hitTestBehavior: HitTestBehavior.opaque,
      dragAnchorStrategy: (draggable, context, position) {
        data.anchor = childDragAnchorStrategy(draggable, context, position);
        return data.anchor;
      },
      onDragStarted: onDragStarted,
      // These callbacks also run after a lazy source card leaves the viewport.
      onDraggableCanceled: (_, _) => onDragEnd(),
      onDragCompleted: onDragEnd,
      feedback: Material(
        key: const ValueKey('reorder-card-proxy'),
        color: Colors.transparent,
        elevation: 6,
        borderRadius: BorderRadius.circular(14),
        child: SizedBox.fromSize(size: size, child: feedback),
      ),
      // Competing with a tap keeps mouse-down alone from starting a drag.
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () {},
        child: child,
      ),
    ),
  );
}
