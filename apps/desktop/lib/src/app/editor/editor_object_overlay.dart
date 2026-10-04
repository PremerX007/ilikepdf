import 'package:flutter/material.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';

/// Core-projected display rectangles share the page stack with its raster.
/// Subtracting the stack origin is placement only, never PDF coordinate math.
class EditorObjectOverlay extends StatelessWidget {
  const EditorObjectOverlay({
    required this.page,
    required this.objects,
    super.key,
  });
  final EditorPageLayout page;
  final List<EditorObjectDisplay> objects;
  static const handleSize = 8.0;

  @override
  Widget build(BuildContext context) => IgnorePointer(
    child: Stack(
      clipBehavior: Clip.none,
      children: [
        for (final object in objects)
          Positioned(
            left: object.rect.left - page.rect.left,
            top: object.rect.top - page.rect.top,
            width: object.rect.width,
            height: object.rect.height,
            child: Semantics(
              label: 'Prototype rectangle',
              selected: object.selected,
              child: DecoratedBox(
                key: ValueKey('editor-object-${object.id}'),
                decoration: BoxDecoration(
                  color: const Color(0x22507BB8),
                  border: Border.all(color: const Color(0xFF507BB8)),
                ),
              ),
            ),
          ),
        for (final object in objects.where((o) => o.selected)) ...[
          Positioned(
            left: object.rect.left - page.rect.left,
            top: object.rect.top - page.rect.top,
            width: object.rect.width,
            height: object.rect.height,
            child: DecoratedBox(
              key: ValueKey('editor-selection-${object.id}'),
              decoration: BoxDecoration(
                border: Border.all(color: const Color(0xFF285EA8), width: 1.5),
              ),
            ),
          ),
          for (var i = 0; i < object.handles.length; i++)
            Positioned(
              left: object.handles[i].x - page.rect.left - handleSize / 2,
              top: object.handles[i].y - page.rect.top - handleSize / 2,
              width: handleSize,
              height: handleSize,
              child: DecoratedBox(
                key: ValueKey('editor-handle-${object.id}-$i'),
                decoration: BoxDecoration(
                  color: Colors.white,
                  border: Border.all(color: const Color(0xFF285EA8)),
                ),
              ),
            ),
        ],
      ],
    ),
  );
}
