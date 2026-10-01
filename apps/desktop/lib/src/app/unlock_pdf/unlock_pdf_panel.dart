import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ilikepdf/src/app/shared/destination_picker.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/file_preview_card.dart';
import 'package:ilikepdf/src/app/shared/pdf_encryption_state.dart';
import 'package:ilikepdf/src/app/shared/pdf_output_name.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';
import 'package:ilikepdf/src/app/unlock_pdf/unlock_pdf_workflow.dart';

class UnlockPdfPanel extends StatefulWidget {
  const UnlockPdfPanel({required this.workflow, super.key});

  final UnlockPdfWorkflow workflow;

  @override
  State<UnlockPdfPanel> createState() => _UnlockPdfPanelState();
}

class _UnlockPdfPanelState extends State<UnlockPdfPanel> {
  final _passwordController = TextEditingController();
  final _outputNameController = TextEditingController();

  SelectedUnlockPdf? _source;
  RenderedUnlockPdfPage? _preview;
  String? _destinationDirectory;
  bool _customDestination = false;
  bool _isSelecting = false;
  bool _isUnlocking = false;
  bool _isPreviewing = false;
  bool _previewFailed = false;
  bool _obscurePassword = true;
  String? _interactionError;
  UnlockPdfUpdate? _update;

  bool get _isBusy => _isSelecting || _isUnlocking;

  String? get _normalizedOutputName =>
      normalizePdfOutputName(_outputNameController.text);

  bool get _passwordTransportValid =>
      !_passwordController.text.contains(RegExp(r'[\x00\r\n]'));

  bool get _canUnlock {
    final source = _source;
    if (_isBusy ||
        source == null ||
        _destinationDirectory == null ||
        _normalizedOutputName == null) {
      return false;
    }
    return switch (source.encryptionState) {
      PdfEncryptionState.unencrypted => false,
      PdfEncryptionState.encryptedNoPasswordRequired => true,
      PdfEncryptionState.encryptedPasswordRequired =>
        _passwordController.text.isNotEmpty && _passwordTransportValid,
    };
  }

  @override
  void dispose() {
    _passwordController.clear();
    _passwordController.dispose();
    _outputNameController.dispose();
    super.dispose();
  }

  Future<void> _selectPdf() async {
    if (_isBusy) return;
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _update = null;
    });
    try {
      final selected = await widget.workflow.selectPdf();
      if (mounted && selected != null) await _replaceSource(selected);
    } on UnlockPdfSelectionException catch (error) {
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
      _update = null;
    });
    try {
      final selected = await widget.workflow.preparePdfPaths(paths);
      if (mounted) await _replaceSource(selected);
    } on UnlockPdfSelectionException catch (error) {
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

  Future<void> _replaceSource(SelectedUnlockPdf selected) async {
    _passwordController.clear();
    setState(() {
      _source = selected;
      _preview = null;
      _previewFailed = false;
      _outputNameController.text = selected.defaultOutputName;
      if (!_customDestination) {
        _destinationDirectory = selected.sourceDirectory;
      }
      _interactionError = null;
      _update = null;
    });
    if (selected.encryptionState !=
        PdfEncryptionState.encryptedPasswordRequired) {
      await _loadPreview(selected);
    } else {
      setState(() => _previewFailed = true);
    }
  }

  Future<void> _loadPreview(SelectedUnlockPdf source) async {
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
    _passwordController.clear();
    setState(() {
      _source = null;
      _preview = null;
      _previewFailed = false;
      _isPreviewing = false;
      _destinationDirectory = null;
      _customDestination = false;
      _outputNameController.clear();
      _interactionError = null;
      _update = null;
    });
  }

  Future<void> _chooseDestination() async {
    if (_isBusy || _source == null) return;
    setState(() => _isSelecting = true);
    try {
      final directory = await widget.workflow.chooseDestinationDirectory();
      if (mounted && directory != null) {
        setState(() {
          _destinationDirectory = directory;
          _customDestination = true;
          _interactionError = null;
          _update = null;
        });
      }
    } on Object {
      if (mounted) {
        setState(
          () => _interactionError = 'The destination could not be selected.',
        );
      }
    } finally {
      if (mounted) setState(() => _isSelecting = false);
    }
  }

  void _fieldChanged(String _) {
    setState(() {
      _interactionError = null;
      _update = null;
    });
  }

  Future<void> _unlock() async {
    final source = _source;
    final destination = _destinationDirectory;
    final outputName = _normalizedOutputName;
    if (!_canUnlock ||
        source == null ||
        destination == null ||
        outputName == null) {
      return;
    }
    final password =
        source.encryptionState == PdfEncryptionState.encryptedPasswordRequired
        ? _passwordController.text
        : null;
    setState(() {
      _isUnlocking = true;
      _interactionError = null;
      _outputNameController.text = outputName;
      _update = const UnlockPdfUpdate(
        status: UnlockPdfUpdateStatus.running,
        stage: UnlockPdfProgressStage.preparing,
        pageCount: 0,
        outputPath: null,
        hasWarnings: false,
        error: null,
      );
    });
    try {
      await for (final update in widget.workflow.unlock(
        sourcePath: source.sourcePath,
        destinationDirectory: destination,
        outputName: outputName,
        password: password,
      )) {
        if (!mounted) continue;
        setState(() {
          _update = update;
          if (update.status == UnlockPdfUpdateStatus.complete) {
            _passwordController.clear();
          }
        });
      }
      if (mounted && _update?.status == UnlockPdfUpdateStatus.running) {
        setState(
          () => _interactionError =
              'The unlock operation ended without a result.',
        );
      }
    } on Object {
      if (mounted) {
        setState(() => _interactionError = 'The PDF could not be unlocked.');
      }
    } finally {
      if (mounted) setState(() => _isUnlocking = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ToolWorkspace(
      title: 'Unlock PDF',
      description: 'Create an unencrypted copy of one PDF without rasterizing its pages.',
      workspace: FileDropZone(
        enabled: !_isBusy,
        onDroppedPaths: _acceptDroppedPaths,
        onTap: _source == null && !_isBusy ? _selectPdf : null,
        child: _source == null
            ? EmptyFileDropContent(
                key: const ValueKey('empty-unlock-pdf-drop-zone'),
                title: 'Drop a PDF here',
                formats: 'PDF · One document at a time',
                actionLabel: 'Select PDF',
                onAction: _selectPdf,
                icon: Icons.lock_open_rounded,
                enabled: !_isBusy,
              )
            : _buildWorkspace(),
      ),
      settings: _buildSettings(),
      status: _buildStatus(),
      primaryAction: PrimaryToolAction(
        key: const ValueKey('unlock-pdf-action'),
        label: 'Unlock PDF',
        runningLabel: 'Unlocking PDF…',
        icon: Icons.lock_open_rounded,
        isRunning: _isUnlocking,
        onPressed: _canUnlock ? _unlock : null,
      ),
    );
  }

  Widget _buildWorkspace() {
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
                key: const ValueKey('replace-unlock-pdf'),
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
                  key: const ValueKey('unlock-pdf-source-card'),
                  thumbnail: _previewWidget(),
                  filename: source.displayName,
                  positionLabel: source.pageCount == null
                      ? 'Password required'
                      : '${source.pageCount} ${source.pageCount == 1 ? 'page' : 'pages'}',
                  removeButtonKey: const ValueKey('remove-unlock-pdf'),
                  onRemove: _isBusy ? null : _clearSource,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _previewWidget() {
    if (_isPreviewing) {
      return const Center(
        child: SizedBox.square(
          dimension: 24,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    }
    if (_previewFailed) {
      return const _PreviewUnavailable(
        key: ValueKey('unlock-pdf-preview-unavailable'),
      );
    }
    if (_preview case final preview?) {
      return Image.file(
        File(preview.outputPath),
        key: const ValueKey('unlock-pdf-preview-image'),
        fit: BoxFit.contain,
        errorBuilder: (_, _, _) => const _PreviewUnavailable(),
      );
    }
    return const _PreviewUnavailable();
  }

  Widget _buildSettings() {
    final source = _source;
    final invalidTransport =
        _passwordController.text.isNotEmpty && !_passwordTransportValid;
    final invalidName =
        _outputNameController.text.isNotEmpty && _normalizedOutputName == null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Encryption status',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 14),
        if (source == null)
          const Text('Select one PDF to inspect its encryption state.')
        else
          switch (source.encryptionState) {
            PdfEncryptionState.unencrypted => const _StateNotice(
              key: ValueKey('unlock-already-unlocked'),
              icon: Icons.lock_open_rounded,
              title: 'This PDF is already unlocked',
              message: 'No output copy is needed.',
            ),
            PdfEncryptionState.encryptedNoPasswordRequired =>
              const _StateNotice(
                key: ValueKey('unlock-no-password-required'),
                icon: Icons.lock_open_rounded,
                title: 'No password required',
                message: 'This PDF can be unlocked directly.',
              ),
            PdfEncryptionState.encryptedPasswordRequired => Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const _StateNotice(
                  key: ValueKey('unlock-password-required'),
                  icon: Icons.password_rounded,
                  title: 'Password required',
                  message: 'Enter the password used to open this PDF.',
                ),
                const SizedBox(height: 14),
                TextField(
                  key: const ValueKey('unlock-password'),
                  controller: _passwordController,
                  enabled: !_isBusy,
                  obscureText: _obscurePassword,
                  enableSuggestions: false,
                  autocorrect: false,
                  decoration: InputDecoration(
                    labelText: 'Password',
                    border: const OutlineInputBorder(),
                    isDense: true,
                    errorText: invalidTransport
                        ? 'Passwords cannot contain line breaks.'
                        : null,
                    suffixIcon: IconButton(
                      key: const ValueKey('toggle-unlock-password-visibility'),
                      tooltip: _obscurePassword
                          ? 'Show password'
                          : 'Hide password',
                      onPressed: _isBusy
                          ? null
                          : () => setState(
                              () => _obscurePassword = !_obscurePassword,
                            ),
                      icon: Icon(
                        _obscurePassword
                            ? Icons.visibility_outlined
                            : Icons.visibility_off_outlined,
                      ),
                    ),
                  ),
                  onChanged: _fieldChanged,
                ),
              ],
            ),
          },
        const SizedBox(height: 20),
        Text('Output filename', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        TextField(
          key: const ValueKey('unlock-output-name'),
          controller: _outputNameController,
          enabled: !_isBusy && source != null,
          decoration: InputDecoration(
            border: const OutlineInputBorder(),
            isDense: true,
            errorText: invalidName ? 'Enter a filename, not a path.' : null,
          ),
          onChanged: _fieldChanged,
        ),
        const SizedBox(height: 18),
        DestinationPicker(
          path: _destinationDirectory,
          placeholder: 'Select a PDF to set the output folder',
          onChoose: _isBusy || source == null ? null : _chooseDestination,
        ),
        if (_destinationDirectory != null) ...[
          const SizedBox(height: 8),
          Text(
            _customDestination
                ? 'Custom destination stays selected when the source changes.'
                : 'Default: source PDF folder.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ],
    );
  }

  Widget? _buildStatus() {
    if (_interactionError case final error?) {
      return _StatusMessage(message: error, isError: true);
    }
    if (_update case final update?) {
      return switch (update.status) {
        UnlockPdfUpdateStatus.running => _StatusMessage(
          key: ValueKey('unlock-progress-${update.stage.name}'),
          message: _stageLabel(update.stage),
          showProgress: true,
        ),
        UnlockPdfUpdateStatus.failed => _StatusMessage(
          key: const ValueKey('unlock-failure'),
          title: 'Unlock failed',
          message: update.error?.message ?? 'The PDF could not be unlocked.',
          isError: true,
        ),
        UnlockPdfUpdateStatus.complete => _StatusMessage(
          key: const ValueKey('unlock-success'),
          title: 'Unlocked successfully',
          message:
              '${update.pageCount} ${update.pageCount == 1 ? 'page' : 'pages'}\n${update.outputPath}',
        ),
      };
    }
    return null;
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
          Icon(Icons.picture_as_pdf_outlined),
          SizedBox(height: 8),
          Text('Preview unavailable'),
        ],
      ),
    );
  }
}

class _StateNotice extends StatelessWidget {
  const _StateNotice({
    required this.icon,
    required this.title,
    required this.message,
    super.key,
  });

  final IconData icon;
  final String title;
  final String message;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, color: Theme.of(context).colorScheme.primary),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: Theme.of(context).textTheme.titleSmall),
              const SizedBox(height: 4),
              Text(message),
            ],
          ),
        ),
      ],
    );
  }
}

class _StatusMessage extends StatelessWidget {
  const _StatusMessage({
    required this.message,
    this.title,
    this.isError = false,
    this.showProgress = false,
    super.key,
  });

  final String? title;
  final String message;
  final bool isError;
  final bool showProgress;

  @override
  Widget build(BuildContext context) {
    final color = isError
        ? Theme.of(context).colorScheme.error
        : Theme.of(context).colorScheme.primary;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (showProgress)
          const SizedBox.square(
            dimension: 20,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        else
          Icon(
            isError ? Icons.error_outline : Icons.check_circle_outline,
            color: color,
          ),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (title case final title?) ...[
                Text(title, style: Theme.of(context).textTheme.titleSmall),
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

String _stageLabel(UnlockPdfProgressStage stage) => switch (stage) {
  UnlockPdfProgressStage.preparing => 'Preparing PDF…',
  UnlockPdfProgressStage.unlocking => 'Unlocking PDF…',
  UnlockPdfProgressStage.validating => 'Validating unlocked PDF…',
  UnlockPdfProgressStage.publishing => 'Publishing…',
  UnlockPdfProgressStage.completed => 'Completed',
};
