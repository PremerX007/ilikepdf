import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/rust/api/editor.dart' as rust;

class EditorSource {
  const EditorSource(this.path, this.name);
  final String path;
  final String name;
}

abstract interface class EditorWorkflow {
  Future<EditorSource?> selectPdf();
  Future<rust.EditorSession> open(EditorSource source);
}

class LocalEditorWorkflow implements EditorWorkflow {
  const LocalEditorWorkflow();
  @override
  Future<EditorSource?> selectPdf() async {
    final file = await openFile(
      acceptedTypeGroups: const [
        XTypeGroup(label: 'PDF documents', extensions: ['pdf']),
      ],
    );
    return file == null ? null : EditorSource(file.path, file.name);
  }

  @override
  Future<rust.EditorSession> open(EditorSource source) =>
      rust.openEditorSession(sourcePath: source.path);
}
