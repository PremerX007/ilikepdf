import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';
import 'package:ilikepdf/src/app/shared/destination_picker.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';

class PdfToImagePanel extends StatefulWidget {
  const PdfToImagePanel({required this.workflow, super.key});

  final PdfToImageWorkflow workflow;

  @override
  State<PdfToImagePanel> createState() => _PdfToImagePanelState();
}

class _PdfToImagePanelState extends State<PdfToImagePanel> {
  SelectedPdf? _selectedPdf;
  RenderedPdfPage? _renderedPage;
  String? _destinationDirectory;
  PdfImageQuality _quality = PdfImageQuality.standard;
  PdfImageExportUpdate? _exportUpdate;
  String? _interactionError;
  String? _previewError;
  bool _isSelecting = false;
  bool _isPreviewing = false;
  bool _isExporting = false;

  bool get _isBusy => _isSelecting || _isPreviewing || _isExporting;

  Future<void> _selectPdf() async {
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _exportUpdate = null;
    });

    SelectedPdf? selected;
    try {
      selected = await widget.workflow.selectAndInspect();
    } on Object {
      if (mounted) {
        setState(() {
          _interactionError = 'This PDF could not be opened.';
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _isSelecting = false;
        });
      }
    }

    if (!mounted || selected == null) {
      return;
    }
    final inspected = selected;
    setState(() {
      _selectedPdf = inspected;
      _renderedPage = null;
      _previewError = null;
      _destinationDirectory = inspected.sourceDirectory;
      _isPreviewing = inspected.pageCount > 0;
    });
    if (inspected.pageCount > 0) {
      await _renderFirstPage(inspected);
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
        setState(() {
          _destinationDirectory = directory;
        });
      }
    } on Object {
      if (mounted) {
        setState(() {
          _interactionError = 'The destination folder could not be selected.';
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _isSelecting = false;
        });
      }
    }
  }

  Future<void> _exportAllPages() async {
    final selected = _selectedPdf;
    final destination = _destinationDirectory;
    if (selected == null || destination == null) {
      return;
    }

    setState(() {
      _isExporting = true;
      _interactionError = null;
      _exportUpdate = PdfImageExportUpdate(
        status: PdfImageExportStatus.running,
        totalPageCount: selected.pageCount,
        completedPageCount: 0,
        currentPage: selected.pageCount == 0 ? null : 1,
        outputFiles: const [],
        error: null,
      );
    });

    try {
      await for (final update in widget.workflow.exportAllPages(
        sourcePath: selected.sourcePath,
        destinationDirectory: destination,
        quality: _quality,
      )) {
        if (mounted) {
          setState(() {
            _exportUpdate = update;
          });
        }
      }
      if (mounted && _exportUpdate?.status == PdfImageExportStatus.running) {
        setState(() {
          _interactionError = 'The export ended without a result.';
        });
      }
    } on Object {
      if (mounted) {
        setState(() {
          _interactionError = 'The PDF could not be exported.';
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _isExporting = false;
        });
      }
    }
  }

  Future<void> _renderFirstPage(SelectedPdf selected) async {
    try {
      final rendered = await widget.workflow.renderFirstPage(
        selected.sourcePath,
      );
      if (mounted && identical(_selectedPdf, selected)) {
        setState(() {
          _renderedPage = rendered;
          _previewError = null;
        });
      }
    } on Object {
      if (mounted && identical(_selectedPdf, selected)) {
        setState(() {
          _previewError = 'Preview unavailable';
        });
      }
    } finally {
      if (mounted && identical(_selectedPdf, selected)) {
        setState(() {
          _isPreviewing = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final selected = _selectedPdf;
    return ToolWorkspace(
      title: 'PDF to Images',
      description: 'Export every PDF page as a PNG image.',
      workspace: selected == null
          ? Material(
              type: MaterialType.transparency,
              child: InkWell(
                key: const ValueKey('empty-pdf-workspace'),
                onTap: _isBusy ? null : _selectPdf,
                borderRadius: BorderRadius.circular(17),
                child: EmptyFileDropContent(
                  title: 'Choose a PDF to begin',
                  formats: 'One PDF · Every page exported as PNG',
                  actionLabel: 'Select PDF',
                  onAction: _selectPdf,
                  icon: Icons.picture_as_pdf_outlined,
                  enabled: !_isBusy,
                ),
              ),
            )
          : _buildDocumentWorkspace(selected),
      settings: _buildSettings(selected),
      status: _buildStatus(),
      primaryAction: PrimaryToolAction(
        key: const ValueKey('convert-to-images-action'),
        label: 'Convert to images',
        runningLabel: 'Converting…',
        icon: Icons.collections_outlined,
        isRunning: _isExporting,
        onPressed:
            _isBusy ||
                selected == null ||
                selected.pageCount == 0 ||
                _destinationDirectory == null
            ? null
            : _exportAllPages,
      ),
    );
  }

  Widget _buildDocumentWorkspace(SelectedPdf selected) {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  'Selected document',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              OutlinedButton.icon(
                onPressed: _isBusy ? null : _selectPdf,
                icon: const Icon(Icons.swap_horiz_rounded),
                label: const Text('Select PDF'),
              ),
            ],
          ),
          const SizedBox(height: 20),
          _DocumentSummary(
            selected: selected,
            renderedPage: _renderedPage,
            isPreviewing: _isPreviewing,
            previewError: _previewError,
          ),
        ],
      ),
    );
  }

  Widget _buildSettings(SelectedPdf? selected) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Export settings', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 18),
        Text('Export quality', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        _QualityOption(
          title: 'Standard',
          selected: _quality == PdfImageQuality.standard,
          onTap: _isBusy
              ? null
              : () => setState(() {
                  _quality = PdfImageQuality.standard;
                  _exportUpdate = null;
                }),
        ),
        const SizedBox(height: 8),
        _QualityOption(
          title: 'High',
          selected: _quality == PdfImageQuality.highQuality,
          onTap: _isBusy
              ? null
              : () => setState(() {
                  _quality = PdfImageQuality.highQuality;
                  _exportUpdate = null;
                }),
        ),
        const SizedBox(height: 20),
        const Divider(),
        const SizedBox(height: 10),
        DestinationPicker(
          path: _destinationDirectory,
          placeholder: 'Select a PDF to set a destination',
          onChoose: _isBusy || selected == null ? null : _chooseDestination,
        ),
      ],
    );
  }

  Widget? _buildStatus() {
    if (_interactionError case final error?) {
      return _FeedbackMessage(message: error, isError: true);
    }
    if (_exportUpdate case final update?) {
      return _ExportFeedback(update: update);
    }
    return null;
  }
}

class _DocumentSummary extends StatelessWidget {
  const _DocumentSummary({
    required this.selected,
    required this.renderedPage,
    required this.isPreviewing,
    required this.previewError,
  });

  final SelectedPdf selected;
  final RenderedPdfPage? renderedPage;
  final bool isPreviewing;
  final String? previewError;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _PreviewThumbnail(
          renderedPage: renderedPage,
          isPreviewing: isPreviewing,
          previewError: previewError,
          hasPages: selected.pageCount > 0,
        ),
        const SizedBox(width: 20),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SelectableText(
                selected.displayName,
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: 6),
              Text(
                '${selected.pageCount} ${selected.pageCount == 1 ? 'page' : 'pages'}',
              ),
              if (selected.firstPageWidthPoints case final width?)
                Text(
                  'First page: ${width.toStringAsFixed(0)} × ${selected.firstPageHeightPoints?.toStringAsFixed(0)} points',
                ),
            ],
          ),
        ),
      ],
    );
  }
}

class _PreviewThumbnail extends StatelessWidget {
  const _PreviewThumbnail({
    required this.renderedPage,
    required this.isPreviewing,
    required this.previewError,
    required this.hasPages,
  });

  final RenderedPdfPage? renderedPage;
  final bool isPreviewing;
  final String? previewError;
  final bool hasPages;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return SizedBox(
      key: const ValueKey('pdf-preview-thumbnail'),
      width: 132,
      height: 168,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colors.surfaceContainerHighest,
          border: Border.all(color: colors.outlineVariant),
          borderRadius: BorderRadius.circular(8),
        ),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(7),
          child: _content(),
        ),
      ),
    );
  }

  Widget _content() {
    if (isPreviewing) {
      return const Center(
        key: ValueKey('pdf-preview-loading'),
        child: SizedBox(
          width: 24,
          height: 24,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    }
    if (previewError != null) {
      return const _PreviewUnavailable(
        key: ValueKey('pdf-preview-error'),
        message: 'Preview unavailable',
      );
    }
    if (renderedPage case final page?) {
      return Image.file(
        File(page.outputPath),
        key: const ValueKey('pdf-preview-image'),
        fit: BoxFit.contain,
        errorBuilder: (context, error, stackTrace) => const _PreviewUnavailable(
          key: ValueKey('pdf-preview-display-error'),
          message: 'Preview unavailable',
        ),
      );
    }
    return _PreviewUnavailable(
      message: hasPages ? 'Preview unavailable' : 'No pages',
    );
  }
}

class _PreviewUnavailable extends StatelessWidget {
  const _PreviewUnavailable({required this.message, super.key});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.insert_drive_file_outlined),
            const SizedBox(height: 8),
            Text(
              message,
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
    required this.selected,
    required this.onTap,
  });

  final String title;
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
          key: ValueKey(title),
          borderRadius: BorderRadius.circular(12),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.all(14),
            child: Row(
              children: [
                Icon(
                  selected
                      ? Icons.radio_button_checked
                      : Icons.radio_button_unchecked,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    title,
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ExportFeedback extends StatelessWidget {
  const _ExportFeedback({required this.update});

  final PdfImageExportUpdate update;

  @override
  Widget build(BuildContext context) {
    return switch (update.status) {
      PdfImageExportStatus.running => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          LinearProgressIndicator(
            value: update.totalPageCount == 0
                ? null
                : update.completedPageCount / update.totalPageCount,
          ),
          const SizedBox(height: 8),
          Text(
            'Exporting page ${update.currentPage ?? 1} of ${update.totalPageCount} · '
            '${update.completedPageCount} completed',
          ),
        ],
      ),
      PdfImageExportStatus.complete => _FeedbackMessage(
        message:
            'Export complete: ${update.completedPageCount} PNG ${update.completedPageCount == 1 ? 'image' : 'images'} created.',
        isError: false,
      ),
      PdfImageExportStatus.failed => _FeedbackMessage(
        message: _failureMessage(update),
        isError: true,
      ),
    };
  }

  String _failureMessage(PdfImageExportUpdate update) {
    final page = update.currentPage == null
        ? ''
        : ' on page ${update.currentPage}';
    final created = update.completedPageCount == 0
        ? 'No images were created.'
        : '${update.completedPageCount} images were created before the failure.';
    return 'Export failed$page. ${update.error?.message ?? 'Unable to complete the export.'} $created';
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
      decoration: BoxDecoration(
        color: isError ? colors.errorContainer : colors.primaryContainer,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Padding(
        padding: const EdgeInsets.all(14),
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
