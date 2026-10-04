import 'package:flutter/material.dart';
import 'package:ilikepdf/src/rust/api/editor.dart';
import 'package:ilikepdf/src/rust/api/error.dart';

import 'editor_render_cache.dart';
import 'editor_viewport.dart';
import 'editor_workflow.dart';

class EditorPanel extends StatefulWidget {
  const EditorPanel({this.workflow = const LocalEditorWorkflow(), super.key});
  final EditorWorkflow workflow;
  @override
  State<EditorPanel> createState() => _EditorPanelState();
}

class _EditorPanelState extends State<EditorPanel> {
  final _cache = EditorRenderCache();
  EditorSession? _session;
  EditorEdits? _edits;
  String? _name;
  String? _error;
  bool _opening = false;
  int _openGeneration = 0;

  Future<void> _open() async {
    final generation = ++_openGeneration;
    try {
      final source = await widget.workflow.selectPdf();
      if (!mounted || generation != _openGeneration || source == null) {
        return;
      }
      setState(() {
        _opening = true;
        _error = null;
      });
      final session = await widget.workflow.open(source);
      if (!mounted || generation != _openGeneration) return;
      final edits = session.createEdits();
      _cache.bind(session);
      setState(() {
        _session = session;
        _edits = edits;
        _name = source.name;
      });
    } on ApplicationError catch (problem) {
      if (mounted && generation == _openGeneration) {
        setState(() => _error = problem.message);
      }
    } on Object {
      if (mounted && generation == _openGeneration) {
        setState(() => _error = 'This PDF could not be opened.');
      }
    } finally {
      if (mounted && generation == _openGeneration) {
        setState(() => _opening = false);
      }
    }
  }

  void _close() {
    _openGeneration++;
    _cache.bind(null);
    setState(() {
      _session = null;
      _edits = null;
      _name = null;
      _error = null;
      _opening = false;
    });
  }

  @override
  void dispose() {
    _openGeneration++;
    _cache.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Column(
    children: [
      Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Editor foundation · Internal preview',
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  Text(
                    _session == null
                        ? 'Open a PDF to view its pages.'
                        : '$_name · ${_session!.pageCount()} pages',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
            OutlinedButton.icon(
              key: const ValueKey('editor-open'),
              onPressed: _opening ? null : _open,
              icon: const Icon(Icons.folder_open_outlined),
              label: Text(_session == null ? 'Open PDF' : 'Replace PDF'),
            ),
            if (_session != null || _opening)
              IconButton(
                key: const ValueKey('editor-close'),
                onPressed: _close,
                tooltip: 'Close PDF',
                icon: const Icon(Icons.close),
              ),
          ],
        ),
      ),
      if (_opening)
        const LinearProgressIndicator(key: ValueKey('editor-opening')),
      if (_error != null)
        Padding(
          padding: const EdgeInsets.all(12),
          child: Text(_error!, key: const ValueKey('editor-open-error')),
        ),
      Expanded(
        child: _session == null
            ? const Center(
                child: Text(
                  'In-memory object prototype. Source PDF remains unchanged.',
                ),
              )
            : EditorViewport(session: _session!, cache: _cache, edits: _edits),
      ),
    ],
  );
}
