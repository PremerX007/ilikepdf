import 'package:ilikepdf/src/rust/api/editor.dart';

enum EditorZoomMode { custom, fitWidth }

/// Typed presentation intent. Core validates and resolves all numerical scales.
class EditorViewportZoom {
  const EditorViewportZoom({required this.scale, required this.mode});
  const EditorViewportZoom.initial()
    : scale = 1,
      mode = EditorZoomMode.fitWidth;
  const EditorViewportZoom.actualSize()
    : scale = 1,
      mode = EditorZoomMode.custom;
  final double scale;
  final EditorZoomMode mode;
  bool get fitsWidth => mode == EditorZoomMode.fitWidth;
  bool get canIncrease => scale < 4;
  bool get canDecrease => scale > 0.25;
  EditorViewportZoom step(bool increase) => EditorViewportZoom(
    scale: stepEditorZoom(scale: scale, increase: increase),
    mode: EditorZoomMode.custom,
  );
  EditorViewportZoom fitWidth() =>
      EditorViewportZoom(scale: scale, mode: EditorZoomMode.fitWidth);
  EditorViewportZoom resolved(double scale) =>
      EditorViewportZoom(scale: scale, mode: mode);
}
