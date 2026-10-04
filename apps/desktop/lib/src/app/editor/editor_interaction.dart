import 'package:flutter/foundation.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

/// Presentation transaction only. Rust owns objects, clamping and all history.
class EditorInteraction extends ChangeNotifier {
  EditorInteraction(this.edits) : snapshot = edits.snapshot();
  final EditorEdits edits;
  EditorEditSnapshot snapshot;
  EditorObjectGesture? _gesture;
  int? _pointer;
  EditorObjectDisplay? preview;
  int? previewPage;
  String? error;

  void _refresh() {
    snapshot = edits.snapshot();
    notifyListeners();
  }

  void perform(VoidCallback action) {
    cancel(notify: false);
    error = null;
    try {
      action();
    } on ApplicationError catch (problem) {
      error = problem.message;
    }
    _refresh();
  }

  void begin(EditorLayoutBinding layout, EditorPoint point, int pointer) {
    if (_pointer != null) return;
    error = null;
    try {
      _gesture = edits.beginGesture(layout: layout, point: point);
      if (_gesture != null) {
        _pointer = pointer;
        previewPage = _gesture!.pageIndex();
      }
    } on ApplicationError catch (problem) {
      error = problem.message;
    }
    _refresh();
  }

  void update(EditorPoint point, int pointer) {
    if (_pointer != pointer || _gesture == null) return;
    try {
      preview = _gesture!.preview(point: point);
    } on ApplicationError catch (problem) {
      error = problem.message;
      cancel(notify: false);
    }
    notifyListeners();
  }

  void finish(EditorPoint point, int pointer) {
    if (_pointer != pointer || _gesture == null) return;
    final gesture = _gesture!;
    perform(() => edits.finishGesture(gesture: gesture, point: point));
  }

  void cancel({bool notify = true}) {
    _gesture = null;
    _pointer = null;
    preview = null;
    previewPage = null;
    if (notify) notifyListeners();
  }
}
