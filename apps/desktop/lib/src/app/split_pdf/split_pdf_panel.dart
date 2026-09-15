import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ilikepdf/src/app/shared/destination_picker.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/file_preview_card.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';
import 'package:ilikepdf/src/app/split_pdf/split_pdf_workflow.dart';

class SplitPdfPanel extends StatefulWidget {
  const SplitPdfPanel({required this.workflow, super.key});

  final SplitPdfWorkflow workflow;

  @override
  State<SplitPdfPanel> createState() => _SplitPdfPanelState();
}

class _SplitPdfPanelState extends State<SplitPdfPanel> {
  static const _maximumMbValue = 18446744073709;

  final TextEditingController _everyNController = TextEditingController(
    text: '2',
  );
  final TextEditingController _splitAfterController = TextEditingController();
  final TextEditingController _maximumSizeController = TextEditingController(
    text: '10',
  );

  SelectedSplitPdf? _source;
  RenderedSplitPdfPage? _preview;
  SplitPdfMode _mode = SplitPdfMode.everyPage;
  String? _destinationDirectory;
  bool _customDestination = false;
  bool _isSelecting = false;
  bool _isSplitting = false;
  bool _isPreviewing = false;
  bool _previewFailed = false;
  String? _interactionError;
  SplitPdfUpdate? _splitUpdate;

  bool get _isBusy => _isSelecting || _isSplitting;

  int? get _everyN => parsePositiveWholeNumber(_everyNController.text);
  int? get _maximumSizeMb =>
      parsePositiveWholeNumber(_maximumSizeController.text);

  List<SplitPdfRange>? get _deterministicRanges {
    final source = _source;
    if (source == null) return null;
    return deriveDeterministicSplitRanges(
      pageCount: source.pageCount,
      mode: _mode,
      everyNPages: _everyNController.text,
      splitAfterPages: _splitAfterController.text,
    );
  }

  String? get _configurationError {
    final source = _source;
    if (source == null) return null;
    if (source.pageCount < 2) {
      return 'This PDF has only one page and cannot be split.';
    }
    switch (_mode) {
      case SplitPdfMode.everyPage:
        return null;
      case SplitPdfMode.everyNPages:
        final every = _everyN;
        if (every == null) return 'Enter a positive whole number of pages.';
        if (every >= source.pageCount) {
          return 'Enter a value smaller than ${source.pageCount}.';
        }
        return null;
      case SplitPdfMode.splitAfterPages:
        if (_splitAfterController.text.trim().isEmpty) {
          return 'Enter at least one split point.';
        }
        if (_deterministicRanges == null) {
          return 'Use ascending, comma-separated pages from 1 to ${source.pageCount - 1}.';
        }
        return null;
      case SplitPdfMode.maximumFileSize:
        final maximumMb = _maximumSizeMb;
        if (maximumMb == null || maximumMb > _maximumMbValue) {
          return 'Enter a positive whole-number size in MB.';
        }
        final limitBytes = maximumMb * 1000000;
        if (source.sizeBytes <= limitBytes) {
          return 'This PDF is already below the $maximumMb MB limit.';
        }
        return null;
    }
  }

  bool get _canSplit =>
      !_isBusy &&
      _source != null &&
      _destinationDirectory != null &&
      _configurationError == null;

  @override
  void dispose() {
    _everyNController.dispose();
    _splitAfterController.dispose();
    _maximumSizeController.dispose();
    super.dispose();
  }

  Future<void> _selectPdf() async {
    if (_isBusy) return;
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _splitUpdate = null;
    });
    try {
      final selected = await widget.workflow.selectPdf();
      if (mounted && selected != null) await _replaceSource(selected);
    } on SplitPdfSelectionException catch (error) {
      if (mounted) setState(() => _interactionError = error.problem.message);
    } on Object {
      if (mounted) {
        setState(() => _interactionError = 'The PDF could not be selected.');
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
      _splitUpdate = null;
    });
    try {
      final selected = await widget.workflow.preparePdfPaths(paths);
      if (mounted) await _replaceSource(selected);
    } on SplitPdfSelectionException catch (error) {
      if (mounted) setState(() => _interactionError = error.problem.message);
    } on Object {
      if (mounted) {
        setState(
          () => _interactionError = 'The dropped PDF could not be opened.',
        );
      }
    } finally {
      if (mounted) setState(() => _isSelecting = false);
    }
  }

  Future<void> _replaceSource(SelectedSplitPdf selected) async {
    setState(() {
      _source = selected;
      _preview = null;
      _previewFailed = false;
      if (!_customDestination) {
        _destinationDirectory = selected.sourceDirectory;
      }
      _interactionError = null;
      _splitUpdate = null;
    });
    await _loadPreview(selected);
  }

  Future<void> _loadPreview(SelectedSplitPdf source) async {
    setState(() => _isPreviewing = true);
    try {
      final preview = await widget.workflow.renderFirstPage(source.sourcePath);
      if (mounted && _source?.sourcePath == source.sourcePath) {
        setState(() {
          _preview = preview;
          _previewFailed = false;
        });
      }
    } on Object {
      if (mounted && _source?.sourcePath == source.sourcePath) {
        setState(() => _previewFailed = true);
      }
    } finally {
      if (mounted && _source?.sourcePath == source.sourcePath) {
        setState(() => _isPreviewing = false);
      }
    }
  }

  void _clearSource() {
    if (_isBusy) return;
    setState(() {
      _source = null;
      _preview = null;
      _previewFailed = false;
      _isPreviewing = false;
      _destinationDirectory = null;
      _customDestination = false;
      _mode = SplitPdfMode.everyPage;
      _everyNController.text = '2';
      _splitAfterController.clear();
      _maximumSizeController.text = '10';
      _interactionError = null;
      _splitUpdate = null;
    });
  }

  Future<void> _chooseDestination() async {
    if (_isBusy || _source == null) return;
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _splitUpdate = null;
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

  void _selectMode(SplitPdfMode mode) {
    if (_isBusy || _mode == mode) return;
    setState(() {
      _mode = mode;
      _interactionError = null;
      _splitUpdate = null;
    });
  }

  void _parametersChanged() {
    setState(() {
      _interactionError = null;
      _splitUpdate = null;
    });
  }

  Future<void> _split() async {
    final source = _source;
    final destination = _destinationDirectory;
    if (!_canSplit || source == null || destination == null) return;
    setState(() {
      _isSplitting = true;
      _interactionError = null;
      _splitUpdate = SplitPdfUpdate(
        status: SplitPdfUpdateStatus.running,
        stage: SplitPdfProgressStage.preparing,
        sourcePageCount: source.pageCount,
        currentPart: null,
        totalParts: _mode == SplitPdfMode.everyPage
            ? source.pageCount
            : _deterministicRanges?.length,
        outputDirectory: null,
        parts: const [],
        hasWarnings: false,
        failedPageNumber: null,
        actualSizeBytes: null,
        limitSizeBytes: null,
        error: null,
      );
    });
    try {
      await for (final update in widget.workflow.split(
        sourcePath: source.sourcePath,
        destinationDirectory: destination,
        mode: _mode,
        everyNPages: _mode == SplitPdfMode.everyNPages ? _everyN : null,
        splitAfterPages: _mode == SplitPdfMode.splitAfterPages
            ? _splitAfterController.text
            : null,
        maximumSizeMb: _mode == SplitPdfMode.maximumFileSize
            ? _maximumSizeMb
            : null,
      )) {
        if (mounted) setState(() => _splitUpdate = update);
      }
      if (mounted && _splitUpdate?.status == SplitPdfUpdateStatus.running) {
        setState(() => _interactionError = 'The split ended without a result.');
      }
    } on Object {
      if (mounted) {
        setState(() => _interactionError = 'The PDF could not be split.');
      }
    } finally {
      if (mounted) setState(() => _isSplitting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ToolWorkspace(
      title: 'Split PDF',
      description: 'Divide one PDF into ordered documents without rasterizing its pages.',
      workspace: FileDropZone(
        enabled: !_isBusy,
        onDroppedPaths: _acceptDroppedPaths,
        onTap: _source == null && !_isBusy ? _selectPdf : null,
        child: _source == null
            ? EmptyFileDropContent(
                key: const ValueKey('empty-split-pdf-drop-zone'),
                title: 'Drop a PDF here',
                formats: 'PDF · One document at a time',
                actionLabel: 'Select PDF',
                onAction: _selectPdf,
                icon: Icons.content_cut_rounded,
                enabled: !_isBusy,
              )
            : _buildPopulatedWorkspace(),
      ),
      settings: _buildSettings(),
      status: _buildStatus(),
      primaryAction: PrimaryToolAction(
        key: const ValueKey('split-pdf-action'),
        label: 'Split PDF',
        runningLabel: 'Splitting PDF…',
        icon: Icons.content_cut_rounded,
        isRunning: _isSplitting,
        onPressed: _canSplit ? _split : null,
      ),
    );
  }

  Widget _buildPopulatedWorkspace() {
    final source = _source!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 14, 12),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  '1 selected PDF',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              OutlinedButton.icon(
                key: const ValueKey('replace-split-pdf'),
                onPressed: _isBusy ? null : _selectPdf,
                icon: const Icon(Icons.swap_horiz_rounded),
                label: const Text('Replace PDF'),
              ),
            ],
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: Center(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(20),
              child: SizedBox(
                width: 280,
                height: 330,
                child: FilePreviewCard(
                  key: const ValueKey('split-pdf-source-card'),
                  thumbnail: _SplitPdfThumbnail(
                    preview: _preview,
                    isPreviewing: _isPreviewing,
                    previewFailed: _previewFailed,
                  ),
                  filename: source.displayName,
                  positionLabel:
                      '${source.pageCount} ${source.pageCount == 1 ? 'page' : 'pages'}',
                  removeButtonKey: const ValueKey('remove-split-pdf'),
                  onRemove: _isBusy ? null : _clearSource,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildSettings() {
    final source = _source;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Split settings', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 16),
        Text('Mode', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            _modeChip(SplitPdfMode.everyPage, 'Every page'),
            _modeChip(SplitPdfMode.everyNPages, 'Every N pages'),
            _modeChip(SplitPdfMode.splitAfterPages, 'Split after pages'),
            _modeChip(SplitPdfMode.maximumFileSize, 'By maximum file size'),
          ],
        ),
        if (_mode == SplitPdfMode.everyNPages) ...[
          const SizedBox(height: 16),
          TextField(
            key: const ValueKey('split-every-n-input'),
            controller: _everyNController,
            enabled: !_isBusy,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              labelText: 'Split every',
              suffixText: 'pages',
              border: OutlineInputBorder(),
              isDense: true,
            ),
            onChanged: (_) => _parametersChanged(),
          ),
        ],
        if (_mode == SplitPdfMode.splitAfterPages) ...[
          const SizedBox(height: 16),
          TextField(
            key: const ValueKey('split-after-input'),
            controller: _splitAfterController,
            enabled: !_isBusy,
            decoration: const InputDecoration(
              labelText: 'Split after pages',
              hintText: '3, 7, 15',
              border: OutlineInputBorder(),
              isDense: true,
            ),
            onChanged: (_) => _parametersChanged(),
          ),
        ],
        if (_mode == SplitPdfMode.maximumFileSize) ...[
          const SizedBox(height: 16),
          TextField(
            key: const ValueKey('split-maximum-size-input'),
            controller: _maximumSizeController,
            enabled: !_isBusy,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              labelText: 'Maximum size per PDF',
              suffixText: 'MB',
              border: OutlineInputBorder(),
              isDense: true,
            ),
            onChanged: (_) => _parametersChanged(),
          ),
        ],
        if (_configurationError case final error?) ...[
          const SizedBox(height: 10),
          Text(
            error,
            key: const ValueKey('split-configuration-error'),
            style: Theme.of(context).textTheme.bodySmall
                ?.copyWith(color: Theme.of(context).colorScheme.error),
          ),
        ],
        const SizedBox(height: 20),
        DestinationPicker(
          path: _destinationDirectory,
          placeholder: 'Select a PDF to set the initial destination',
          onChoose: _isBusy || source == null ? null : _chooseDestination,
        ),
        const SizedBox(height: 8),
        Text(
          _customDestination
              ? 'Custom destination · output uses a <name>-split folder'
              : 'Defaults to the source folder · output uses a <name>-split folder',
          key: const ValueKey('split-destination-mode-label'),
          style: Theme.of(context).textTheme.bodySmall
              ?.copyWith(color: Theme.of(context).colorScheme.onSurfaceVariant),
        ),
        const SizedBox(height: 20),
        const Divider(),
        const SizedBox(height: 10),
        Text('Output summary', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 6),
        _buildOutputSummary(),
      ],
    );
  }

  Widget _modeChip(SplitPdfMode mode, String label) {
    return ChoiceChip(
      key: ValueKey('split-mode-${mode.name}'),
      label: Text(label),
      selected: _mode == mode,
      onSelected: _isBusy ? null : (_) => _selectMode(mode),
    );
  }

  Widget _buildOutputSummary() {
    final source = _source;
    if (source == null) {
      return const Text('Select one PDF to calculate the output.');
    }
    if (_mode == SplitPdfMode.maximumFileSize) {
      final maximum = _maximumSizeMb;
      return Text(
        [
          '${source.pageCount} pages',
          if (maximum != null) 'Maximum $maximum MB per PDF',
          'Parts will be determined during splitting.',
        ].join('\n'),
        key: const ValueKey('split-output-summary'),
      );
    }
    if (_mode == SplitPdfMode.everyPage) {
      return Text(
        '${source.pageCount} pages\n${source.pageCount} PDF files',
        key: const ValueKey('split-output-summary'),
      );
    }
    final ranges = _deterministicRanges;
    if (ranges == null) {
      return const Text(
        'Enter valid split settings to calculate the output.',
        key: ValueKey('split-output-summary'),
      );
    }
    final details = <String>[
      '${source.pageCount} pages',
      '${ranges.length} PDF files',
    ];
    if (_mode != SplitPdfMode.everyPage && ranges.length <= 8) {
      for (var index = 0; index < ranges.length; index++) {
        details.add('Part ${index + 1}  ${_rangeLabel(ranges[index])}');
      }
    }
    return Text(
      details.join('\n'),
      key: const ValueKey('split-output-summary'),
    );
  }

  Widget? _buildStatus() {
    if (_interactionError case final error?) {
      return _SplitFeedback(message: error, isError: true);
    }
    if (_splitUpdate case final update?) {
      if (update.status == SplitPdfUpdateStatus.running) {
        return _SplitProgress(update: update);
      }
      if (update.status == SplitPdfUpdateStatus.failed) {
        return _SplitFeedback(
          key: const ValueKey('split-failure'),
          title: 'Split failed',
          message: update.error?.message ?? 'The PDF could not be split.',
          isError: true,
        );
      }
      return _SplitSuccess(update: update);
    }
    return null;
  }
}

class _SplitPdfThumbnail extends StatelessWidget {
  const _SplitPdfThumbnail({
    required this.preview,
    required this.isPreviewing,
    required this.previewFailed,
  });

  final RenderedSplitPdfPage? preview;
  final bool isPreviewing;
  final bool previewFailed;

  @override
  Widget build(BuildContext context) {
    if (isPreviewing) {
      return const Center(
        key: ValueKey('split-pdf-preview-loading'),
        child: SizedBox.square(
          dimension: 24,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    }
    if (previewFailed) {
      return const _PreviewUnavailable(
        key: ValueKey('split-pdf-preview-error'),
      );
    }
    if (preview case final rendered?) {
      return Image.file(
        File(rendered.outputPath),
        key: const ValueKey('split-pdf-preview-image'),
        fit: BoxFit.contain,
        errorBuilder: (_, _, _) => const _PreviewUnavailable(),
      );
    }
    return const _PreviewUnavailable();
  }
}

class _PreviewUnavailable extends StatelessWidget {
  const _PreviewUnavailable({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.broken_image_outlined),
          SizedBox(height: 8),
          Text('Preview unavailable'),
        ],
      ),
    );
  }
}

class _SplitProgress extends StatelessWidget {
  const _SplitProgress({required this.update});

  final SplitPdfUpdate update;

  @override
  Widget build(BuildContext context) {
    final current = update.currentPart;
    final total = update.totalParts;
    final message = switch (update.stage) {
      SplitPdfProgressStage.preparing => 'Preparing PDF…',
      SplitPdfProgressStage.findingSplitPoints => 'Finding split points…',
      SplitPdfProgressStage.creating when current != null && total != null =>
        'Creating part $current of $total…',
      SplitPdfProgressStage.creating when current != null =>
        'Creating part $current…',
      SplitPdfProgressStage.creating => 'Creating split PDFs…',
      SplitPdfProgressStage.validating => 'Validating output…',
      SplitPdfProgressStage.publishing => 'Publishing files…',
      SplitPdfProgressStage.completed => 'Completed',
    };
    return Row(
      key: ValueKey('split-progress-${update.stage.name}'),
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

class _SplitSuccess extends StatelessWidget {
  const _SplitSuccess({required this.update});

  final SplitPdfUpdate update;

  @override
  Widget build(BuildContext context) {
    return Column(
      key: const ValueKey('split-success'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Split completed',
          style: Theme.of(context).textTheme.titleSmall
              ?.copyWith(color: Theme.of(context).colorScheme.primary),
        ),
        const SizedBox(height: 5),
        Text('${update.parts.length} PDF files'),
        const SizedBox(height: 5),
        for (var index = 0; index < update.parts.length.clamp(0, 12); index++)
          Text(
            'Part ${index + 1}  ${_rangeLabel(update.parts[index].range)}'
            '${update.parts[index].sizeBytes > 0 ? '  ${_formatMb(update.parts[index].sizeBytes)}' : ''}',
          ),
        if (update.parts.length > 12)
          Text('${update.parts.length - 12} more PDF files'),
        if (update.hasWarnings) ...[
          const SizedBox(height: 5),
          const Text(
            'The source PDF contained recoverable structural warnings.',
          ),
        ],
        if (update.outputDirectory case final outputDirectory?) ...[
          const SizedBox(height: 5),
          Text(outputDirectory),
        ],
      ],
    );
  }
}

class _SplitFeedback extends StatelessWidget {
  const _SplitFeedback({
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
    final color = isError
        ? Theme.of(context).colorScheme.error
        : Theme.of(context).colorScheme.primary;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (title case final visibleTitle?) ...[
          Text(
            visibleTitle,
            style: Theme.of(context).textTheme.titleSmall
                ?.copyWith(color: color),
          ),
          const SizedBox(height: 5),
        ],
        Text(message, style: TextStyle(color: color)),
      ],
    );
  }
}

String _rangeLabel(SplitPdfRange range) => range.firstPage == range.lastPage
    ? 'Page ${range.firstPage}'
    : 'Pages ${range.firstPage}–${range.lastPage}';

String _formatMb(int bytes) => '${(bytes / 1000000).toStringAsFixed(1)} MB';
