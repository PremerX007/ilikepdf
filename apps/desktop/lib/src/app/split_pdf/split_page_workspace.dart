import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;
import 'package:ilikepdf/src/app/shared/page_thumbnail_cache.dart';
import 'package:ilikepdf/src/app/split_pdf/split_pdf_workflow.dart';

/// Virtualizes rows *inside* parts, so even a single huge range stays lazy.
/// Layout arithmetic only; output ranges always arrive from the Rust planner.
class SplitPageWorkspace extends StatefulWidget {
  const SplitPageWorkspace({
    required this.cache,
    required this.pageCount,
    required this.ranges,
    required this.compactParts,
    required this.boundaryMode,
    required this.canToggle,
    required this.onToggle,
    this.partSizes,
    super.key,
  });

  final PageThumbnailCache cache;
  final int pageCount;
  final List<SplitPdfRange>? ranges;
  final bool compactParts;
  final bool boundaryMode;
  final bool canToggle;
  final ValueChanged<int> onToggle;
  final List<int>? partSizes;

  @override
  State<SplitPageWorkspace> createState() => _SplitPageWorkspaceState();
}

class _SplitPageWorkspaceState extends State<SplitPageWorkspace> {
  _PartRows? _layout;
  int? _columns;

  @override
  void didUpdateWidget(SplitPageWorkspace oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(widget.ranges, oldWidget.ranges) ||
        widget.pageCount != oldWidget.pageCount ||
        widget.compactParts != oldWidget.compactParts) {
      _layout = null;
    }
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final ranges = widget.ranges;
      final partSizes = widget.partSizes;
      final compactParts = widget.compactParts;
      final pageCount = widget.pageCount;
      final columns = ((constraints.maxWidth - 56) / 156).floor().clamp(1, 5);
      // At most six intersecting rows / 30 live images, even on tall displays.
      // This leaves room in the 32-entry cache for in-flight scroll results.
      final rowExtent = math.max(260.0, constraints.maxHeight / 5);
      final grouped = ranges != null && !compactParts;
      if (_columns != columns) _layout = null;
      _columns = columns;
      final layout = _layout ??= _PartRows(
        grouped ? ranges : [SplitPdfRange(firstPage: 1, lastPage: pageCount)],
        columns,
      );
      return ListView.builder(
        key: const ValueKey('split-page-workspace'),
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
        itemExtent: rowExtent,
        scrollCacheExtent: const ScrollCacheExtent.pixels(0),
        itemCount: layout.rowCount,
        itemBuilder: (context, rowIndex) {
          final (partIndex, rowInPart) = layout.locate(rowIndex);
          final range = layout.ranges[partIndex];
          final first = range.firstPage + rowInPart * columns;
          final last = math.min(first + columns - 1, range.lastPage);
          final firstRow = first == range.firstPage;
          final lastRow = last == range.lastPage;
          final label = 'Part ${partIndex + 1} · ${splitRangeLabel(range)}';
          return Padding(
            padding: EdgeInsets.only(bottom: lastRow ? 12 : 0),
            child: CustomPaint(
              painter: grouped
                  ? _DashedPartBorder(
                      color: Theme.of(context).colorScheme.outlineVariant,
                      top: firstRow,
                      bottom: lastRow,
                    )
                  : null,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 8, 12, 10),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (grouped)
                      Semantics(
                        key: ValueKey(
                          'split-part-${partIndex + 1}-row-$rowInPart',
                        ),
                        container: true,
                        header: true,
                        child: Padding(
                          padding: const EdgeInsets.only(bottom: 8),
                          child: Text(
                            '$label${firstRow ? '' : ' · continued'}'
                            '${partSizes == null ? '' : ' · ${(partSizes[partIndex] / 1000000).toStringAsFixed(1)} MB'}',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: Theme.of(context).textTheme.labelLarge,
                          ),
                        ),
                      ),
                    Expanded(
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          for (var column = 0; column < columns; column++) ...[
                            if (column != 0) const SizedBox(width: 12),
                            Expanded(
                              child: first + column > last
                                  ? const SizedBox.shrink()
                                  : _page(
                                      first + column,
                                      boundary:
                                          grouped &&
                                          first + column == range.lastPage &&
                                          range.lastPage < pageCount,
                                      part: compactParts && ranges != null
                                          ? first + column
                                          : null,
                                    ),
                            ),
                          ],
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          );
        },
      );
    },
  );

  Widget _page(int page, {required bool boundary, required int? part}) =>
      _SplitPageCard(
        key: ValueKey('split-page-$page'),
        cache: widget.cache,
        page: page,
        part: part,
        boundaryMode: widget.boundaryMode,
        boundary: widget.boundaryMode && boundary,
        finalPage: page == widget.pageCount,
        onToggle:
            widget.boundaryMode && widget.canToggle && page < widget.pageCount
            ? () => widget.onToggle(page)
            : null,
      );
}

/// Only a prefix count per output part is retained, never a widget per page.
class _PartRows {
  _PartRows(this.ranges, int columns) {
    for (final range in ranges) {
      ends.add(
        (ends.isEmpty ? 0 : ends.last) +
            (range.pageCount + columns - 1) ~/ columns,
      );
    }
  }

  final List<SplitPdfRange> ranges;
  final List<int> ends = [];
  int get rowCount => ends.last;

  (int, int) locate(int row) {
    var low = 0;
    var high = ends.length - 1;
    while (low < high) {
      final middle = (low + high) ~/ 2;
      if (row < ends[middle]) {
        high = middle;
      } else {
        low = middle + 1;
      }
    }
    return (low, row - (low == 0 ? 0 : ends[low - 1]));
  }
}

class _SplitPageCard extends StatelessWidget {
  const _SplitPageCard({
    required this.cache,
    required this.page,
    required this.part,
    required this.boundaryMode,
    required this.boundary,
    required this.finalPage,
    required this.onToggle,
    super.key,
  });

  final PageThumbnailCache cache;
  final int page;
  final int? part;
  final bool boundaryMode;
  final bool boundary;
  final bool finalPage;
  final VoidCallback? onToggle;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final hint = finalPage
        ? 'Final page — no split boundary needed'
        : 'Split after page $page';
    return Semantics(
      label: boundaryMode
          ? 'Page $page. $hint'
          : 'Page $page${part == null ? '' : '. Part $part, one PDF'}',
      button: boundaryMode && !finalPage,
      enabled: boundaryMode ? onToggle != null : null,
      toggled: boundaryMode && !finalPage ? boundary : null,
      child: Tooltip(
        message: boundaryMode ? hint : 'Page $page',
        child: Material(
          color: colors.surface,
          clipBehavior: Clip.antiAlias,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(10),
            side: BorderSide(
              color: boundary ? colors.primary : colors.outlineVariant,
              width: boundary ? 2 : 1,
            ),
          ),
          child: InkWell(
            onTap: onToggle,
            child: Padding(
              padding: const EdgeInsets.all(8),
              child: Column(
                children: [
                  if (part != null)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 6),
                      child: Text(
                        'Part $part · 1 PDF',
                        key: ValueKey('split-output-$part'),
                        style: Theme.of(context).textTheme.labelMedium,
                      ),
                    ),
                  Expanded(
                    child: ExcludeSemantics(
                      child: LazyPageThumbnail(
                        key: ValueKey('split-thumbnail-$page'),
                        cache: cache,
                        pageIndex: page - 1,
                      ),
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Page $page',
                    style: Theme.of(context).textTheme.labelLarge,
                  ),
                  if (boundaryMode) ...[
                    const SizedBox(height: 4),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(
                          finalPage
                              ? Icons.last_page_rounded
                              : boundary
                              ? Icons.check_circle_rounded
                              : Icons.content_cut_rounded,
                          size: 14,
                          color: boundary
                              ? colors.primary
                              : colors.onSurfaceVariant,
                        ),
                        const SizedBox(width: 4),
                        Flexible(
                          child: Text(
                            finalPage ? 'Final page' : 'Split after $page',
                            key: ValueKey(
                              boundary
                                  ? 'split-boundary-$page'
                                  : 'split-boundary-option-$page',
                            ),
                            style: Theme.of(context).textTheme.labelSmall,
                          ),
                        ),
                      ],
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _DashedPartBorder extends CustomPainter {
  const _DashedPartBorder({
    required this.color,
    required this.top,
    required this.bottom,
  });
  final Color color;
  final bool top;
  final bool bottom;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..strokeWidth = 1;
    void dashed(Offset start, Offset end) {
      final distance = (end - start).distance;
      if (distance == 0) return;
      final unit = (end - start) / distance;
      for (var offset = 0.0; offset < distance; offset += 10) {
        canvas.drawLine(
          start + unit * offset,
          start + unit * math.min(offset + 5, distance),
          paint,
        );
      }
    }

    dashed(const Offset(0.5, 0), Offset(0.5, size.height));
    dashed(Offset(size.width - 0.5, 0), Offset(size.width - 0.5, size.height));
    if (top) dashed(const Offset(0, 0.5), Offset(size.width, 0.5));
    if (bottom) {
      dashed(
        Offset(0, size.height - 0.5),
        Offset(size.width, size.height - 0.5),
      );
    }
  }

  @override
  bool shouldRepaint(_DashedPartBorder oldDelegate) =>
      color != oldDelegate.color ||
      top != oldDelegate.top ||
      bottom != oldDelegate.bottom;
}

String splitRangeLabel(SplitPdfRange range) => range.firstPage == range.lastPage
    ? 'Page ${range.firstPage}'
    : 'Pages ${range.firstPage}–${range.lastPage}';
