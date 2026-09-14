import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ilikepdf/src/app/merge_pdf/merge_pdf_workflow.dart';
import 'package:ilikepdf/src/app/shared/destination_picker.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/file_preview_card.dart';
import 'package:ilikepdf/src/app/shared/reorderable_item_grid.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';

class MergePdfPanel extends StatefulWidget {
  const MergePdfPanel({required this.workflow, super.key});

  final MergePdfWorkflow workflow;

  @override
  State<MergePdfPanel> createState() => _MergePdfPanelState();
}

class _MergePdfItem {
  const _MergePdfItem({required this.id, required this.pdf});

  final int id;
  final SelectedMergePdf pdf;
}

class _MergePdfPanelState extends State<MergePdfPanel> {
  final List<_MergePdfItem> _pdfs = [];
  final Map<int, RenderedMergePdfPage> _previews = {};
  final Set<int> _previewing = {};
  final Set<int> _previewFailures = {};
  final TextEditingController _outputNameController = TextEditingController(
    text: 'merged.pdf',
  );
  int _nextItemId = 0;
  String? _destinationDirectory;
  bool _customDestination = false;
  bool _isSelecting = false;
  bool _isMerging = false;
  String? _interactionError;
  MergePdfUpdate? _mergeUpdate;

  bool get _isBusy => _isSelecting || _isMerging;
  int get _totalPages =>
      _pdfs.fold(0, (total, item) => total + item.pdf.pageCount);
  String? get _normalizedOutputName =>
      normalizeMergeOutputName(_outputNameController.text);
  bool get _canMerge =>
      !_isBusy &&
      _pdfs.length >= 2 &&
      _destinationDirectory != null &&
      _normalizedOutputName != null;

  @override
  void dispose() {
    _outputNameController.dispose();
    super.dispose();
  }

  Future<void> _selectPdfs() async {
    if (_isBusy) return;
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _mergeUpdate = null;
    });
    try {
      final selected = await widget.workflow.selectPdfs();
      if (mounted && selected.isNotEmpty) await _appendPdfs(selected);
    } on MergePdfSelectionException catch (error) {
      if (mounted) setState(() => _interactionError = error.problem.message);
    } on Object {
      if (mounted) {
        setState(() => _interactionError = 'The PDFs could not be selected.');
      }
    } finally {
      if (mounted) setState(() => _isSelecting = false);
    }
  }

  Future<void> _acceptDroppedPaths(List<String> paths) async {
    if (_isBusy) return;
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _mergeUpdate = null;
    });
    try {
      final selected = await widget.workflow.preparePdfPaths(paths);
      if (mounted && selected.isNotEmpty) await _appendPdfs(selected);
    } on MergePdfSelectionException catch (error) {
      if (mounted) setState(() => _interactionError = error.problem.message);
    } on Object {
      if (mounted) {
        setState(
          () => _interactionError = 'The dropped PDFs could not be added.',
        );
      }
    } finally {
      if (mounted) setState(() => _isSelecting = false);
    }
  }

  Future<void> _appendPdfs(List<SelectedMergePdf> selected) async {
    final wasEmpty = _pdfs.isEmpty;
    final added = <_MergePdfItem>[];
    setState(() {
      for (final pdf in selected) {
        final item = _MergePdfItem(id: _nextItemId++, pdf: pdf);
        _pdfs.add(item);
        added.add(item);
      }
      if (wasEmpty && !_customDestination) {
        _destinationDirectory = selected.first.sourceDirectory;
      }
      _interactionError = null;
      _mergeUpdate = null;
    });
    await _loadPreviews(added);
  }

  Future<void> _loadPreviews(List<_MergePdfItem> items) async {
    for (final item in items) {
      if (!mounted ||
          item.pdf.pageCount == 0 ||
          item.pdf.inspectionProblem != null) {
        continue;
      }
      setState(() => _previewing.add(item.id));
      try {
        final preview = await widget.workflow.renderFirstPage(
          item.pdf.sourcePath,
        );
        if (mounted && _pdfs.any((candidate) => candidate.id == item.id)) {
          setState(() {
            _previews[item.id] = preview;
            _previewFailures.remove(item.id);
          });
        }
      } on Object {
        if (mounted && _pdfs.any((candidate) => candidate.id == item.id)) {
          setState(() => _previewFailures.add(item.id));
        }
      } finally {
        if (mounted) setState(() => _previewing.remove(item.id));
      }
    }
  }

  Future<void> _chooseDestination() async {
    if (_isBusy || _pdfs.isEmpty) return;
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _mergeUpdate = null;
    });
    try {
      final directory = await widget.workflow.chooseDestinationDirectory();
      if (mounted && directory != null) {
        setState(() {
          _destinationDirectory = directory;
          _customDestination = true;
        });
      }
    } on Object {
      if (mounted) {
        setState(
          () => _interactionError =
              'The destination folder could not be selected.',
        );
      }
    } finally {
      if (mounted) setState(() => _isSelecting = false);
    }
  }

  void _removePdf(int index) {
    setState(() {
      final removed = _pdfs.removeAt(index);
      _previews.remove(removed.id);
      _previewing.remove(removed.id);
      _previewFailures.remove(removed.id);
      _mergeUpdate = null;
      if (_pdfs.isEmpty) {
        _destinationDirectory = null;
        _customDestination = false;
        _outputNameController.text = 'merged.pdf';
      }
    });
  }

  void _reorder(int oldIndex, int newIndex) {
    if (oldIndex == newIndex) return;
    setState(() {
      final item = _pdfs.removeAt(oldIndex);
      _pdfs.insert(newIndex, item);
      _mergeUpdate = null;
    });
  }

  Future<void> _merge() async {
    final outputName = _normalizedOutputName;
    final destination = _destinationDirectory;
    if (!_canMerge || outputName == null || destination == null) return;
    setState(() {
      _isMerging = true;
      _interactionError = null;
      _outputNameController.text = outputName;
      _mergeUpdate = MergePdfUpdate(
        status: MergePdfUpdateStatus.running,
        stage: MergePdfProgressStage.preparing,
        inputCount: _pdfs.length,
        totalPageCount: _totalPages,
        outputPath: null,
        warningInputCount: 0,
        hasWarnings: false,
        failedInputIndex: null,
        failedInputPath: null,
        error: null,
      );
    });
    try {
      await for (final update in widget.workflow.merge(
        sourcePaths: _pdfs
            .map((item) => item.pdf.sourcePath)
            .toList(growable: false),
        destinationDirectory: destination,
        outputName: outputName,
      )) {
        if (mounted) setState(() => _mergeUpdate = update);
      }
      if (mounted && _mergeUpdate?.status == MergePdfUpdateStatus.running) {
        setState(() => _interactionError = 'The merge ended without a result.');
      }
    } on Object {
      if (mounted) {
        setState(() => _interactionError = 'The PDFs could not be merged.');
      }
    } finally {
      if (mounted) setState(() => _isMerging = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ToolWorkspace(
      title: 'Merge PDF',
      description:
          'Combine every page from your PDFs in the card order you choose.',
      workspace: FileDropZone(
        enabled: !_isBusy,
        onDroppedPaths: _acceptDroppedPaths,
        onTap: _pdfs.isEmpty && !_isBusy ? _selectPdfs : null,
        child: _pdfs.isEmpty
            ? EmptyFileDropContent(
                key: const ValueKey('empty-merge-pdf-drop-zone'),
                title: 'Drop PDFs here',
                formats: 'PDF · Select at least two documents',
                actionLabel: 'Select PDFs',
                onAction: _selectPdfs,
                icon: Icons.call_merge_rounded,
                enabled: !_isBusy,
              )
            : _buildPopulatedWorkspace(),
      ),
      settings: _buildSettings(),
      status: _buildStatus(),
      primaryAction: PrimaryToolAction(
        key: const ValueKey('merge-pdf-action'),
        label: 'Merge PDF',
        runningLabel: 'Merging PDF…',
        icon: Icons.call_merge_rounded,
        isRunning: _isMerging,
        onPressed: _canMerge ? _merge : null,
      ),
    );
  }

  Widget _buildPopulatedWorkspace() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 14, 12),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  '${_pdfs.length} selected ${_pdfs.length == 1 ? 'PDF' : 'PDFs'}',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              Text(
                'Drag cards to reorder',
                style: Theme.of(context).textTheme.bodySmall,
              ),
              const SizedBox(width: 12),
              OutlinedButton.icon(
                onPressed: _isBusy ? null : _selectPdfs,
                icon: const Icon(Icons.add_rounded),
                label: const Text('Add PDFs'),
              ),
            ],
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(16),
            child: ReorderableItemGrid<_MergePdfItem>(
              items: _pdfs,
              itemHeight: 238,
              enabled: !_isBusy,
              onReorder: _reorder,
              itemBuilder: (context, item, index, reorderHandle) =>
                  FilePreviewCard(
                    key: ValueKey('merge-pdf-card-${item.id}'),
                    thumbnail: _MergePdfThumbnail(
                      pdf: item.pdf,
                      preview: _previews[item.id],
                      isPreviewing: _previewing.contains(item.id),
                      previewFailed: _previewFailures.contains(item.id),
                    ),
                    filename: item.pdf.displayName,
                    positionLabel:
                        'PDF ${index + 1} · ${_pageCountLabel(item.pdf)}',
                    reorderHandle: KeyedSubtree(
                      key: ValueKey('reorder-merge-pdf-$index'),
                      child: reorderHandle,
                    ),
                    removeButtonKey: ValueKey('remove-merge-pdf-$index'),
                    onRemove: _isBusy ? null : () => _removePdf(index),
                  ),
            ),
          ),
        ),
      ],
    );
  }

  String _pageCountLabel(SelectedMergePdf pdf) {
    if (pdf.inspectionProblem != null) return 'Cannot open';
    return '${pdf.pageCount} ${pdf.pageCount == 1 ? 'page' : 'pages'}';
  }

  Widget _buildSettings() {
    final nameIsInvalid =
        _outputNameController.text.isNotEmpty && _normalizedOutputName == null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Merge settings', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 18),
        Text('Output filename', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        TextField(
          key: const ValueKey('merge-output-name'),
          controller: _outputNameController,
          enabled: !_isBusy,
          decoration: InputDecoration(
            border: const OutlineInputBorder(),
            isDense: true,
            errorText: nameIsInvalid ? 'Enter a filename, not a path.' : null,
          ),
          onChanged: (_) => setState(() {
            _interactionError = null;
            _mergeUpdate = null;
          }),
          onEditingComplete: () {
            final normalized = _normalizedOutputName;
            if (normalized != null) {
              setState(() => _outputNameController.text = normalized);
            }
            FocusScope.of(context).unfocus();
          },
        ),
        const SizedBox(height: 20),
        DestinationPicker(
          path: _destinationDirectory,
          placeholder: 'Select PDFs to set the initial destination',
          onChoose: _isBusy || _pdfs.isEmpty ? null : _chooseDestination,
        ),
        const SizedBox(height: 8),
        Text(
          _customDestination
              ? 'Custom destination'
              : 'Defaults to the initial first PDF folder',
          key: const ValueKey('merge-destination-mode-label'),
          style: Theme.of(context).textTheme.bodySmall
              ?.copyWith(color: Theme.of(context).colorScheme.onSurfaceVariant),
        ),
        const SizedBox(height: 20),
        const Divider(),
        const SizedBox(height: 10),
        Text('Summary', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 6),
        Text(
          '${_pdfs.length} ${_pdfs.length == 1 ? 'PDF' : 'PDFs'} • '
          '$_totalPages ${_totalPages == 1 ? 'page' : 'pages'}',
          key: const ValueKey('merge-input-summary'),
          style: Theme.of(context).textTheme.bodyLarge,
        ),
      ],
    );
  }

  Widget? _buildStatus() {
    if (_interactionError case final error?) {
      return _MergeFeedback(message: error, isError: true);
    }
    if (_mergeUpdate case final update?) {
      if (update.status == MergePdfUpdateStatus.running) {
        return _MergeProgress(update: update);
      }
      if (update.status == MergePdfUpdateStatus.failed) {
        final inputName = update.failedInputPath == null
            ? null
            : File(update.failedInputPath!).uri.pathSegments.last;
        return _MergeFeedback(
          key: const ValueKey('merge-failure'),
          title: 'Merge failed',
          message: [
            ?inputName,
            update.error?.message ?? 'The PDFs could not be merged.',
          ].join('\n'),
          isError: true,
        );
      }
      return _MergeFeedback(
        key: const ValueKey('merge-success'),
        title: 'Merged successfully',
        message: [
          '${update.inputCount} PDFs · ${update.totalPageCount} pages',
          if (update.warningInputCount > 0)
            '${update.warningInputCount} source PDF${update.warningInputCount == 1 ? '' : 's'} contained recoverable structural warnings.',
          if (update.outputPath != null) update.outputPath!,
        ].join('\n'),
        isError: false,
      );
    }
    return null;
  }
}

class _MergePdfThumbnail extends StatelessWidget {
  const _MergePdfThumbnail({
    required this.pdf,
    required this.preview,
    required this.isPreviewing,
    required this.previewFailed,
  });

  final SelectedMergePdf pdf;
  final RenderedMergePdfPage? preview;
  final bool isPreviewing;
  final bool previewFailed;

  @override
  Widget build(BuildContext context) {
    if (pdf.inspectionProblem case final problem?) {
      return _PreviewUnavailable(
        key: const ValueKey('merge-pdf-inspection-error'),
        icon: Icons.error_outline_rounded,
        message: problem.message,
      );
    }
    if (isPreviewing) {
      return const Center(
        key: ValueKey('merge-pdf-preview-loading'),
        child: SizedBox.square(
          dimension: 24,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    }
    if (previewFailed) {
      return const _PreviewUnavailable(
        key: ValueKey('merge-pdf-preview-error'),
        icon: Icons.broken_image_outlined,
        message: 'Preview unavailable',
      );
    }
    if (preview case final rendered?) {
      return Image.file(
        File(rendered.outputPath),
        key: const ValueKey('merge-pdf-preview-image'),
        fit: BoxFit.contain,
        errorBuilder: (_, _, _) => const _PreviewUnavailable(
          icon: Icons.broken_image_outlined,
          message: 'Preview unavailable',
        ),
      );
    }
    return const _PreviewUnavailable(
      icon: Icons.insert_drive_file_outlined,
      message: 'Preview unavailable',
    );
  }
}

class _PreviewUnavailable extends StatelessWidget {
  const _PreviewUnavailable({
    required this.icon,
    required this.message,
    super.key,
  });

  final IconData icon;
  final String message;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon),
            const SizedBox(height: 8),
            Text(
              message,
              maxLines: 3,
              overflow: TextOverflow.ellipsis,
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }
}

class _MergeProgress extends StatelessWidget {
  const _MergeProgress({required this.update});

  final MergePdfUpdate update;

  @override
  Widget build(BuildContext context) {
    final message = switch (update.stage) {
      MergePdfProgressStage.preparing => 'Preparing PDFs…',
      MergePdfProgressStage.merging =>
        'Merging ${update.totalPageCount} pages from ${update.inputCount} PDFs…',
      MergePdfProgressStage.validating => 'Validating merged PDF…',
      MergePdfProgressStage.publishing => 'Publishing output…',
      MergePdfProgressStage.completed => 'Completed',
    };
    return Row(
      key: ValueKey('merge-progress-${update.stage.name}'),
      children: [
        const SizedBox.square(
          dimension: 20,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
        const SizedBox(width: 12),
        Expanded(child: Text(message)),
      ],
    );
  }
}

class _MergeFeedback extends StatelessWidget {
  const _MergeFeedback({
    required this.message,
    required this.isError,
    this.title,
    super.key,
  });

  final String? title;
  final String message;
  final bool isError;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(
          isError ? Icons.error_outline_rounded : Icons.check_circle_outline,
          color: isError ? colors.error : colors.primary,
        ),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (title != null) ...[
                Text(title!, style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 3),
              ],
              Text(message),
            ],
          ),
        ),
      ],
    );
  }
}
