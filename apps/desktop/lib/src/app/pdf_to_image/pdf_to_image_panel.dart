import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';
import 'package:ilikepdf/src/app/shared/destination_picker.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/file_preview_card.dart';
import 'package:ilikepdf/src/app/shared/reorderable_item_grid.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';

class PdfToImagePanel extends StatefulWidget {
  const PdfToImagePanel({required this.workflow, super.key});

  final PdfToImageWorkflow workflow;

  @override
  State<PdfToImagePanel> createState() => _PdfToImagePanelState();
}

class _PdfToImagePanelState extends State<PdfToImagePanel> {
  final List<SelectedPdf> _pdfs = [];
  final Map<String, RenderedPdfPage> _previews = {};
  final Set<String> _previewing = {};
  final Set<String> _previewFailures = {};
  PdfImageQuality _quality = PdfImageQuality.standard;
  PdfImageFormat _format = PdfImageFormat.png;
  PdfDestinationMode _destinationMode = PdfDestinationMode.nextToSourceFiles;
  String? _customDestinationDirectory;
  PdfBatchExportUpdate? _exportUpdate;
  String? _interactionError;
  bool _isSelecting = false;
  bool _isExporting = false;

  bool get _isBusy => _isSelecting || _isExporting;

  Future<void> _selectPdfs() async {
    final selected = await _selectPdfsSafely();
    if (!mounted || selected == null || selected.isEmpty) {
      return;
    }
    await _appendPdfs(selected);
  }

  Future<List<SelectedPdf>?> _selectPdfsSafely() async {
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _exportUpdate = null;
    });
    try {
      return await widget.workflow.selectPdfs();
    } on PdfSelectionException catch (error) {
      if (mounted) {
        setState(() => _interactionError = error.problem.message);
      }
      return null;
    } on Object {
      if (mounted) {
        setState(() => _interactionError = 'The PDFs could not be selected.');
      }
      return null;
    } finally {
      if (mounted) {
        setState(() => _isSelecting = false);
      }
    }
  }

  Future<void> _acceptDroppedPaths(List<String> paths) async {
    if (_isBusy) {
      return;
    }
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _exportUpdate = null;
    });
    try {
      final selected = await widget.workflow.preparePdfPaths(paths);
      if (mounted && selected.isNotEmpty) {
        await _appendPdfs(selected);
      }
    } on PdfSelectionException catch (error) {
      if (mounted) {
        setState(() => _interactionError = error.problem.message);
      }
    } on Object {
      if (mounted) {
        setState(
          () => _interactionError = 'The dropped PDFs could not be added.',
        );
      }
    } finally {
      if (mounted) {
        setState(() => _isSelecting = false);
      }
    }
  }

  Future<void> _appendPdfs(List<SelectedPdf> selected) async {
    final added = <SelectedPdf>[];
    setState(() {
      final existingPaths = _pdfs
          .map((pdf) => pdf.sourcePath.toLowerCase())
          .toSet();
      for (final pdf in selected) {
        if (existingPaths.add(pdf.sourcePath.toLowerCase())) {
          _pdfs.add(pdf);
          added.add(pdf);
        }
      }
      _interactionError = null;
      _exportUpdate = null;
    });
    await _loadPreviews(added);
  }

  Future<void> _loadPreviews(List<SelectedPdf> pdfs) async {
    for (final pdf in pdfs) {
      if (!mounted || pdf.pageCount == 0 || pdf.inspectionProblem != null) {
        continue;
      }
      final key = _pathKey(pdf.sourcePath);
      setState(() => _previewing.add(key));
      try {
        final preview = await widget.workflow.renderFirstPage(pdf.sourcePath);
        if (mounted && _containsPath(key)) {
          setState(() {
            _previews[key] = preview;
            _previewFailures.remove(key);
          });
        }
      } on Object {
        if (mounted && _containsPath(key)) {
          setState(() => _previewFailures.add(key));
        }
      } finally {
        if (mounted) {
          setState(() => _previewing.remove(key));
        }
      }
    }
  }

  Future<void> _chooseDestination() async {
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _exportUpdate = null;
    });
    try {
      final directory = await widget.workflow.chooseDestinationDirectory();
      if (mounted && directory != null) {
        setState(() => _customDestinationDirectory = directory);
      }
    } on Object {
      if (mounted) {
        setState(() {
          _interactionError = 'The destination folder could not be selected.';
        });
      }
    } finally {
      if (mounted) {
        setState(() => _isSelecting = false);
      }
    }
  }

  void _removePdf(int index) {
    setState(() {
      final removed = _pdfs.removeAt(index);
      final key = _pathKey(removed.sourcePath);
      _previews.remove(key);
      _previewing.remove(key);
      _previewFailures.remove(key);
      _exportUpdate = null;
    });
  }

  void _reorder(int oldIndex, int newIndex) {
    if (oldIndex == newIndex) {
      return;
    }
    setState(() {
      final pdf = _pdfs.removeAt(oldIndex);
      _pdfs.insert(newIndex, pdf);
      _exportUpdate = null;
    });
  }

  Future<void> _exportBatch() async {
    if (_pdfs.isEmpty || _isBusy || !_hasValidDestination) {
      return;
    }
    setState(() {
      _isExporting = true;
      _interactionError = null;
      _exportUpdate = PdfBatchExportUpdate(
        status: PdfBatchExportStatus.running,
        totalDocumentCount: _pdfs.length,
        completedDocumentCount: 0,
        succeededDocumentCount: 0,
        failedDocumentCount: 0,
        currentDocumentIndex: 1,
        currentDocumentFilename: _pdfs.first.displayName,
        totalPageCount: _pdfs.fold(0, (sum, pdf) => sum + pdf.pageCount),
        completedPageCount: 0,
        currentPage: _pdfs.first.pageCount == 0 ? null : 1,
        documents: const [],
        error: null,
      );
    });
    try {
      await for (final update in widget.workflow.exportBatch(
        sourcePaths: _pdfs.map((pdf) => pdf.sourcePath).toList(growable: false),
        destinationMode: _destinationMode,
        customDestinationDirectory: _customDestinationDirectory,
        quality: _quality,
        format: _format,
      )) {
        if (mounted) {
          setState(() => _exportUpdate = update);
        }
      }
      if (mounted && _exportUpdate?.status == PdfBatchExportStatus.running) {
        setState(() {
          _interactionError = 'The batch ended without a result.';
        });
      }
    } on Object {
      if (mounted) {
        setState(() => _interactionError = 'The PDFs could not be exported.');
      }
    } finally {
      if (mounted) {
        setState(() => _isExporting = false);
      }
    }
  }

  bool get _hasValidDestination =>
      _destinationMode == PdfDestinationMode.nextToSourceFiles ||
      _customDestinationDirectory != null;

  bool _containsPath(String key) =>
      _pdfs.any((pdf) => _pathKey(pdf.sourcePath) == key);

  String _pathKey(String path) => path.toLowerCase();

  @override
  Widget build(BuildContext context) {
    return ToolWorkspace(
      title: 'PDF to Images',
      description: 'Arrange PDFs and export every page as a PNG image.',
      workspace: FileDropZone(
        enabled: !_isBusy,
        onDroppedPaths: _acceptDroppedPaths,
        onTap: _pdfs.isEmpty && !_isBusy ? _selectPdfs : null,
        child: _pdfs.isEmpty
            ? EmptyFileDropContent(
                key: const ValueKey('empty-pdf-drop-zone'),
                title: 'Drop PDFs here',
                formats: 'PDF · One or more documents',
                actionLabel: 'Select PDFs',
                onAction: _selectPdfs,
                icon: Icons.picture_as_pdf_outlined,
                enabled: !_isBusy,
              )
            : _buildPopulatedWorkspace(),
      ),
      settings: _buildSettings(),
      status: _buildStatus(),
      primaryAction: PrimaryToolAction(
        key: const ValueKey('convert-to-images-action'),
        label: 'Convert to images',
        runningLabel: 'Converting PDFs…',
        icon: Icons.collections_outlined,
        isRunning: _isExporting,
        onPressed: _isBusy || _pdfs.isEmpty || !_hasValidDestination
            ? null
            : _exportBatch,
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
            child: ReorderableItemGrid<SelectedPdf>(
              items: _pdfs,
              itemHeight: 238,
              enabled: !_isBusy,
              onReorder: _reorder,
              itemBuilder: (context, pdf, index, reorderHandle) {
                return FilePreviewCard(
                  key: ValueKey('selected-pdf-${pdf.sourcePath}'),
                  thumbnail: _PdfCardThumbnail(
                    key: ValueKey('thumbnail-${pdf.sourcePath}'),
                    pdf: pdf,
                    preview: _previews[_pathKey(pdf.sourcePath)],
                    isPreviewing: _previewing.contains(
                      _pathKey(pdf.sourcePath),
                    ),
                    previewFailed: _previewFailures.contains(
                      _pathKey(pdf.sourcePath),
                    ),
                  ),
                  filename: pdf.displayName,
                  positionLabel: 'PDF ${index + 1} · ${_pageCountLabel(pdf)}',
                  reorderHandle: KeyedSubtree(
                    key: ValueKey('reorder-pdf-$index'),
                    child: reorderHandle,
                  ),
                  removeButtonKey: ValueKey('remove-pdf-$index'),
                  onRemove: _isBusy ? null : () => _removePdf(index),
                );
              },
            ),
          ),
        ),
      ],
    );
  }

  String _pageCountLabel(SelectedPdf pdf) {
    if (pdf.inspectionProblem != null) {
      return 'Cannot open';
    }
    return '${pdf.pageCount} ${pdf.pageCount == 1 ? 'page' : 'pages'}';
  }

  Widget _buildSettings() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Export settings', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 18),
        Text('Output format', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        SegmentedButton<PdfImageFormat>(
          key: const ValueKey('pdf-output-format'),
          expandedInsets: EdgeInsets.zero,
          showSelectedIcon: false,
          segments: const [
            ButtonSegment(value: PdfImageFormat.png, label: Text('PNG')),
            ButtonSegment(value: PdfImageFormat.jpg, label: Text('JPG')),
          ],
          selected: {_format},
          onSelectionChanged: _isBusy
              ? null
              : (values) => setState(() {
                  _format = values.single;
                  _exportUpdate = null;
                }),
        ),
        const SizedBox(height: 20),
        Text('Export quality', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        Row(
          children: [
            Expanded(
              child: _QualityOption(
                title: 'Standard',
                dpi: 150,
                selected: _quality == PdfImageQuality.standard,
                onTap: _isBusy
                    ? null
                    : () => setState(() {
                        _quality = PdfImageQuality.standard;
                        _exportUpdate = null;
                      }),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: _QualityOption(
                title: 'High',
                dpi: 300,
                selected: _quality == PdfImageQuality.highQuality,
                onTap: _isBusy
                    ? null
                    : () => setState(() {
                        _quality = PdfImageQuality.highQuality;
                        _exportUpdate = null;
                      }),
              ),
            ),
          ],
        ),
        const SizedBox(height: 20),
        const Divider(),
        const SizedBox(height: 10),
        Text('Destination', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        SegmentedButton<PdfDestinationMode>(
          key: const ValueKey('pdf-destination-mode'),
          expandedInsets: EdgeInsets.zero,
          showSelectedIcon: false,
          segments: const [
            ButtonSegment(
              value: PdfDestinationMode.nextToSourceFiles,
              label: _SingleLineLabel('Next to source files'),
            ),
            ButtonSegment(
              value: PdfDestinationMode.customFolder,
              label: _SingleLineLabel('Custom folder'),
            ),
          ],
          selected: {_destinationMode},
          onSelectionChanged: _isBusy
              ? null
              : (values) => setState(() {
                  _destinationMode = values.single;
                  _exportUpdate = null;
                }),
        ),
        const SizedBox(height: 10),
        if (_destinationMode == PdfDestinationMode.nextToSourceFiles)
          Text(
            'Each PDF exports beside its source file.',
            key: const ValueKey('next-to-source-description'),
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          )
        else
          DestinationPicker(
            path: _customDestinationDirectory,
            placeholder: 'Choose a folder for all batch outputs',
            onChoose: _isBusy || _pdfs.isEmpty ? null : _chooseDestination,
            showHeading: false,
          ),
      ],
    );
  }

  Widget? _buildStatus() {
    if (_interactionError case final error?) {
      return _FeedbackMessage(message: error, isError: true);
    }
    if (_exportUpdate case final update?) {
      return _BatchFeedback(update: update);
    }
    return null;
  }
}

class _PdfCardThumbnail extends StatelessWidget {
  const _PdfCardThumbnail({
    required this.pdf,
    required this.preview,
    required this.isPreviewing,
    required this.previewFailed,
    super.key,
  });

  final SelectedPdf pdf;
  final RenderedPdfPage? preview;
  final bool isPreviewing;
  final bool previewFailed;

  @override
  Widget build(BuildContext context) {
    if (pdf.inspectionProblem case final problem?) {
      return _PreviewUnavailable(
        key: const ValueKey('pdf-inspection-error'),
        icon: Icons.error_outline_rounded,
        message: problem.message,
      );
    }
    if (isPreviewing) {
      return const Center(
        key: ValueKey('pdf-preview-loading'),
        child: SizedBox.square(
          dimension: 24,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    }
    if (previewFailed) {
      return const _PreviewUnavailable(
        key: ValueKey('pdf-preview-error'),
        icon: Icons.broken_image_outlined,
        message: 'Preview unavailable',
      );
    }
    if (preview case final rendered?) {
      return Image.file(
        File(rendered.outputPath),
        key: const ValueKey('pdf-preview-image'),
        fit: BoxFit.contain,
        errorBuilder: (context, error, stackTrace) => const _PreviewUnavailable(
          key: ValueKey('pdf-preview-display-error'),
          icon: Icons.broken_image_outlined,
          message: 'Preview unavailable',
        ),
      );
    }
    return _PreviewUnavailable(
      icon: Icons.insert_drive_file_outlined,
      message: pdf.pageCount == 0 ? 'No pages' : 'Preview unavailable',
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

class _QualityOption extends StatelessWidget {
  const _QualityOption({
    required this.title,
    required this.dpi,
    required this.selected,
    required this.onTap,
  });

  final String title;
  final int dpi;
  final bool selected;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Semantics(
      button: true,
      selected: selected,
      child: Material(
        color: selected ? colors.secondaryContainer : colors.surface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: BorderSide(
            color: selected ? colors.primary : colors.outlineVariant,
          ),
        ),
        child: InkWell(
          key: ValueKey('quality-$title'),
          borderRadius: BorderRadius.circular(12),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Icon(
                      selected
                          ? Icons.radio_button_checked
                          : Icons.radio_button_unchecked,
                      size: 18,
                    ),
                    const SizedBox(width: 7),
                    Expanded(
                      child: Text(
                        title,
                        style: Theme.of(context).textTheme.titleSmall,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                Text(
                  'DPI : $dpi',
                  style: Theme.of(context).textTheme.bodySmall
                      ?.copyWith(color: colors.onSurfaceVariant),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _SingleLineLabel extends StatelessWidget {
  const _SingleLineLabel(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return FittedBox(
      fit: BoxFit.scaleDown,
      child: Text(text, maxLines: 1, softWrap: false),
    );
  }
}

class _BatchFeedback extends StatelessWidget {
  const _BatchFeedback({required this.update});

  final PdfBatchExportUpdate update;

  @override
  Widget build(BuildContext context) {
    return switch (update.status) {
      PdfBatchExportStatus.running => _RunningBatchFeedback(update: update),
      PdfBatchExportStatus.complete => _BatchResultSummary(
        key: const ValueKey('batch-summary-success'),
        update: update,
        title: 'Conversion complete',
        isError: false,
      ),
      PdfBatchExportStatus.completeWithErrors => _BatchResultSummary(
        key: const ValueKey('batch-summary-partial'),
        update: update,
        title: 'Completed with errors',
        isError: true,
      ),
      PdfBatchExportStatus.failed => _BatchResultSummary(
        key: const ValueKey('batch-summary-failed'),
        update: update,
        title: 'Batch failed',
        isError: true,
      ),
    };
  }
}

class _RunningBatchFeedback extends StatelessWidget {
  const _RunningBatchFeedback({required this.update});

  final PdfBatchExportUpdate update;

  @override
  Widget build(BuildContext context) {
    final documentIndex = update.currentDocumentIndex ?? 1;
    final overallPage = update.totalPageCount == 0
        ? 0
        : (update.completedPageCount + 1).clamp(1, update.totalPageCount);
    return Column(
      key: const ValueKey('feedback-running'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        LinearProgressIndicator(
          value: update.totalPageCount == 0
              ? null
              : update.completedPageCount / update.totalPageCount,
        ),
        const SizedBox(height: 8),
        Text('Converting PDFs', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 3),
        Text('PDF $documentIndex of ${update.totalDocumentCount}'),
        if (update.currentDocumentFilename case final filename?)
          Text(filename, maxLines: 1, overflow: TextOverflow.ellipsis),
        const SizedBox(height: 3),
        Text('Page $overallPage of ${update.totalPageCount}'),
        if (update.currentPage case final page?)
          Text(
            'Current document page $page',
            style: Theme.of(context).textTheme.bodySmall,
          ),
      ],
    );
  }
}

class _BatchResultSummary extends StatelessWidget {
  const _BatchResultSummary({
    required this.update,
    required this.title,
    required this.isError,
    super.key,
  });

  final PdfBatchExportUpdate update;
  final String title;
  final bool isError;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: isError ? colors.errorContainer : colors.primaryContainer,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(title, style: Theme.of(context).textTheme.titleSmall),
            const SizedBox(height: 4),
            Text(
              '${update.succeededDocumentCount} ${_pdfLabel(update.succeededDocumentCount)} completed · '
              '${update.failedDocumentCount} ${_pdfLabel(update.failedDocumentCount)} failed',
            ),
            if (update.error case final error?) ...[
              const SizedBox(height: 4),
              Text(error.message),
            ],
            for (final document in update.documents) ...[
              const SizedBox(height: 8),
              _DocumentOutcomeRow(document: document),
            ],
          ],
        ),
      ),
    );
  }

  String _pdfLabel(int count) => count == 1 ? 'PDF' : 'PDFs';
}

class _DocumentOutcomeRow extends StatelessWidget {
  const _DocumentOutcomeRow({required this.document});

  final PdfBatchDocumentOutcome document;

  @override
  Widget build(BuildContext context) {
    final succeeded = document.error == null;
    final outputLocation = document.outputFiles.isEmpty
        ? null
        : File(document.outputFiles.first).parent.path;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(
          succeeded ? Icons.check_circle_outline : Icons.error_outline,
          size: 18,
        ),
        const SizedBox(width: 7),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                document.displayName,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.labelLarge,
              ),
              Text(
                succeeded
                    ? '${document.completedPageCount} ${document.completedPageCount == 1 ? 'image' : 'images'} created'
                    : document.error?.message ?? 'Unable to export this PDF',
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.bodySmall,
              ),
              if (outputLocation != null)
                Tooltip(
                  message: outputLocation,
                  child: Text(
                    outputLocation,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}

class _FeedbackMessage extends StatelessWidget {
  const _FeedbackMessage({required this.message, required this.isError});

  final String message;
  final bool isError;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return DecoratedBox(
      key: ValueKey(isError ? 'feedback-error' : 'feedback-success'),
      decoration: BoxDecoration(
        color: isError ? colors.errorContainer : colors.primaryContainer,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Text(
          message,
          style: TextStyle(
            color: isError
                ? colors.onErrorContainer
                : colors.onPrimaryContainer,
          ),
        ),
      ),
    );
  }
}
