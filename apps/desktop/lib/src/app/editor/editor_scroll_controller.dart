import 'package:flutter/widgets.dart';

/// Stages core's anchor until the new content extents are known during layout.
/// Both axes are corrected before paint, without a post-frame jump notification.
class EditorScrollController extends ScrollController {
  EditorScrollController() : super(keepScrollOffset: false);

  double? _pendingOffset;
  double get layoutOffset => _pendingOffset ?? (hasClients ? offset : 0);

  void stageOffset(double offset) {
    // Density-only rebuilds need no scroll layout; do not shadow later scrolling.
    _pendingOffset = hasClients && offset == this.offset ? null : offset;
  }

  @override
  ScrollPosition createScrollPosition(
    ScrollPhysics physics,
    ScrollContext context,
    ScrollPosition? oldPosition,
  ) => _EditorScrollPosition(
    controller: this,
    physics: physics,
    context: context,
    oldPosition: oldPosition,
  );
}

class _EditorScrollPosition extends ScrollPositionWithSingleContext {
  _EditorScrollPosition({
    required this.controller,
    required super.physics,
    required super.context,
    super.oldPosition,
  }) : super(initialPixels: 0, keepScrollOffset: false);

  final EditorScrollController controller;
  bool _applyingAnchor = false;

  @override
  bool applyContentDimensions(double minScrollExtent, double maxScrollExtent) {
    final pending = controller._pendingOffset;
    if (pending == null) {
      return super.applyContentDimensions(minScrollExtent, maxScrollExtent);
    }
    controller._pendingOffset = null;
    final target = pending.clamp(minScrollExtent, maxScrollExtent);
    final changed = target != pixels;
    goIdle();
    correctBy(target - pixels);
    _applyingAnchor = true;
    try {
      final accepted = super.applyContentDimensions(
        minScrollExtent,
        maxScrollExtent,
      );
      return accepted && !changed;
    } finally {
      _applyingAnchor = false;
    }
  }

  @override
  bool correctForNewDimensions(
    ScrollMetrics oldPosition,
    ScrollMetrics newPosition,
  ) =>
      _applyingAnchor ||
      super.correctForNewDimensions(oldPosition, newPosition);
}
