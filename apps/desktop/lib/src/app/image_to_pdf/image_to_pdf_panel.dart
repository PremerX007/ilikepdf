import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ilikepdf/src/app/image_to_pdf/image_to_pdf_workflow.dart';
import 'package:ilikepdf/src/app/shared/destination_picker.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/file_preview_card.dart';
import 'package:ilikepdf/src/app/shared/reorderable_item_grid.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';

class ImageToPdfPanel extends StatefulWidget {
  const ImageToPdfPanel({required this.workflow, super.key});

  final ImageToPdfWorkflow workflow;

  @override
  State<ImageToPdfPanel> createState() => _ImageToPdfPanelState();
}

class _ImageToPdfPanelState extends State<ImageToPdfPanel> {
  final List<SelectedImage> _images = [];
  String? _destinationDirectory;
  ImagePageSize _pageSize = ImagePageSize.a4;
  ImagePageOrientation _orientation = ImagePageOrientation.portrait;
  ImagePageMargin _margin = ImagePageMargin.none;
  ImagePdfCreationUpdate? _creationUpdate;
  String? _interactionError;
  bool _merge = true;
  bool _isSelecting = false;
  bool _isCreating = false;
  bool _hasCustomDestination = false;

  bool get _isBusy => _isSelecting || _isCreating;

  Future<void> _replaceImages() async {
    final selected = await _selectImagesSafely();
    if (!mounted || selected == null || selected.isEmpty) {
      return;
    }
    setState(() {
      _images
        ..clear()
        ..addAll(_uniqueImages(selected));
      _destinationDirectory = _images.first.sourceDirectory;
      _hasCustomDestination = false;
      _creationUpdate = null;
    });
  }

  Future<void> _addImages() async {
    final selected = await _selectImagesSafely();
    if (!mounted || selected == null || selected.isEmpty) {
      return;
    }
    _appendImages(selected);
  }

  Future<void> _acceptDroppedPaths(List<String> paths) async {
    if (_isBusy) {
      return;
    }
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _creationUpdate = null;
    });
    try {
      final selected = await widget.workflow.prepareImagePaths(paths);
      if (mounted && selected.isNotEmpty) {
        _appendImages(selected);
      }
    } on ImageSelectionException catch (error) {
      if (mounted) {
        setState(() => _interactionError = error.problem.message);
      }
    } on Object {
      if (mounted) {
        setState(() {
          _interactionError = 'The dropped images could not be added.';
        });
      }
    } finally {
      if (mounted) {
        setState(() => _isSelecting = false);
      }
    }
  }

  void _appendImages(List<SelectedImage> selected) {
    setState(() {
      final wasEmpty = _images.isEmpty;
      final existing = _images
          .map((image) => image.sourcePath.toLowerCase())
          .toSet();
      _images.addAll(
        selected.where((image) => existing.add(image.sourcePath.toLowerCase())),
      );
      if (wasEmpty && _images.isNotEmpty && !_hasCustomDestination) {
        _destinationDirectory = _images.first.sourceDirectory;
      }
      _creationUpdate = null;
      _interactionError = null;
    });
  }

  List<SelectedImage> _uniqueImages(List<SelectedImage> images) {
    final paths = <String>{};
    return images
        .where((image) => paths.add(image.sourcePath.toLowerCase()))
        .toList(growable: false);
  }

  Future<List<SelectedImage>?> _selectImagesSafely() async {
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _creationUpdate = null;
    });
    try {
      return await widget.workflow.selectImages();
    } on ImageSelectionException catch (error) {
      if (mounted) {
        setState(() => _interactionError = error.problem.message);
      }
      return null;
    } on Object {
      if (mounted) {
        setState(() {
          _interactionError = 'The images could not be selected.';
        });
      }
      return null;
    } finally {
      if (mounted) {
        setState(() => _isSelecting = false);
      }
    }
  }

  Future<void> _chooseDestination() async {
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _creationUpdate = null;
    });
    try {
      final directory = await widget.workflow.chooseDestinationDirectory();
      if (mounted && directory != null) {
        setState(() {
          _destinationDirectory = directory;
          _hasCustomDestination = true;
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
        setState(() => _isSelecting = false);
      }
    }
  }

  void _removeImage(int index) {
    setState(() {
      _images.removeAt(index);
      _creationUpdate = null;
      if (_images.isEmpty && !_hasCustomDestination) {
        _destinationDirectory = null;
      }
    });
  }

  void _reorder(int oldIndex, int newIndex) {
    if (oldIndex == newIndex) {
      return;
    }
    setState(() {
      final image = _images.removeAt(oldIndex);
      _images.insert(newIndex, image);
      _creationUpdate = null;
    });
  }

  Future<void> _createPdfs() async {
    final destination = _destinationDirectory;
    if (_images.isEmpty || destination == null || _isBusy) {
      return;
    }
    setState(() {
      _isCreating = true;
      _interactionError = null;
      _creationUpdate = ImagePdfCreationUpdate(
        status: ImagePdfCreationStatus.running,
        totalImageCount: _images.length,
        completedImageCount: 0,
        currentImage: 1,
        outputFiles: const [],
        error: null,
      );
    });
    try {
      await for (final update in widget.workflow.createPdfs(
        sourcePaths: _images.map((image) => image.sourcePath).toList(),
        destinationDirectory: destination,
        pageSize: _pageSize,
        orientation: _orientation,
        margin: _margin,
        merge: _merge,
      )) {
        if (mounted) {
          setState(() => _creationUpdate = update);
        }
      }
      if (mounted &&
          _creationUpdate?.status == ImagePdfCreationStatus.running) {
        setState(() {
          _interactionError = 'PDF creation ended without a result.';
        });
      }
    } on Object {
      if (mounted) {
        setState(() {
          _interactionError = 'The PDFs could not be created.';
        });
      }
    } finally {
      if (mounted) {
        setState(() => _isCreating = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return ToolWorkspace(
      title: 'Images to PDF',
      description:
          'Arrange JPG, PNG, or WebP images and create one or more PDFs.',
      workspace: FileDropZone(
        enabled: !_isBusy,
        onDroppedPaths: _acceptDroppedPaths,
        onTap: _images.isEmpty && !_isBusy ? _replaceImages : null,
        child: _images.isEmpty
            ? EmptyFileDropContent(
                key: const ValueKey('empty-image-drop-zone'),
                title: 'Drop images here',
                formats: 'JPG · JPEG · PNG · WebP',
                actionLabel: 'Select images',
                onAction: _replaceImages,
                icon: Icons.add_photo_alternate_outlined,
                enabled: !_isBusy,
              )
            : _buildPopulatedWorkspace(),
      ),
      settings: _buildSettings(),
      status: _buildStatus(),
      primaryAction: PrimaryToolAction(
        key: const ValueKey('create-pdf-action'),
        label: _merge ? 'Create PDF' : 'Create PDFs',
        runningLabel: 'Creating PDF…',
        icon: Icons.picture_as_pdf_outlined,
        isRunning: _isCreating,
        onPressed: _isBusy || _images.isEmpty || _destinationDirectory == null
            ? null
            : _createPdfs,
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
                  '${_images.length} selected ${_images.length == 1 ? 'image' : 'images'}',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              Text(
                'Drag cards to reorder',
                style: Theme.of(context).textTheme.bodySmall,
              ),
              const SizedBox(width: 12),
              OutlinedButton.icon(
                onPressed: _isBusy ? null : _addImages,
                icon: const Icon(Icons.add_photo_alternate_outlined),
                label: const Text('Add images'),
              ),
            ],
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(16),
            child: ReorderableItemGrid<SelectedImage>(
              items: _images,
              enabled: !_isBusy,
              onReorder: _reorder,
              itemBuilder: (context, image, index, reorderHandle) {
                return FilePreviewCard(
                  key: ValueKey('selected-image-${image.sourcePath}'),
                  thumbnail: Image.file(
                    File(image.sourcePath),
                    key: ValueKey('thumbnail-${image.sourcePath}'),
                    cacheWidth: 256,
                    cacheHeight: 256,
                    fit: BoxFit.cover,
                    errorBuilder: (context, error, stackTrace) =>
                        const Center(child: Icon(Icons.broken_image_outlined)),
                  ),
                  filename: image.displayName,
                  positionLabel: 'Page ${index + 1}',
                  reorderHandle: KeyedSubtree(
                    key: ValueKey('reorder-image-$index'),
                    child: reorderHandle,
                  ),
                  removeButtonKey: ValueKey('remove-image-$index'),
                  onRemove: _isBusy ? null : () => _removeImage(index),
                );
              },
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildSettings() {
    final destination = _destinationDirectory;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('PDF settings', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 18),
        _OptionsSection(
          pageSize: _pageSize,
          orientation: _orientation,
          margin: _margin,
          enabled: !_isBusy,
          onPageSizeChanged: (value) => setState(() {
            _pageSize = value;
            _creationUpdate = null;
          }),
          onOrientationChanged: (value) => setState(() {
            _orientation = value;
            _creationUpdate = null;
          }),
          onMarginChanged: (value) => setState(() {
            _margin = value;
            _creationUpdate = null;
          }),
        ),
        const SizedBox(height: 14),
        const Divider(),
        CheckboxListTile(
          key: const ValueKey('merge-images-checkbox'),
          contentPadding: EdgeInsets.zero,
          value: _merge,
          onChanged: _isBusy
              ? null
              : (value) => setState(() {
                  _merge = value ?? true;
                  _creationUpdate = null;
                }),
          title: const Text('Merge all images in one PDF file'),
          controlAffinity: ListTileControlAffinity.leading,
        ),
        const Divider(),
        const SizedBox(height: 10),
        DestinationPicker(
          path: destination,
          placeholder: 'Select images to set a destination',
          onChoose: _isBusy || _images.isEmpty ? null : _chooseDestination,
        ),
      ],
    );
  }

  Widget? _buildStatus() {
    if (_interactionError case final error?) {
      return _FeedbackMessage(message: error, isError: true);
    }
    if (_creationUpdate case final update?) {
      return _CreationFeedback(update: update);
    }
    return null;
  }
}

class _OptionsSection extends StatelessWidget {
  const _OptionsSection({
    required this.pageSize,
    required this.orientation,
    required this.margin,
    required this.enabled,
    required this.onPageSizeChanged,
    required this.onOrientationChanged,
    required this.onMarginChanged,
  });

  final ImagePageSize pageSize;
  final ImagePageOrientation orientation;
  final ImagePageMargin margin;
  final bool enabled;
  final ValueChanged<ImagePageSize> onPageSizeChanged;
  final ValueChanged<ImagePageOrientation> onOrientationChanged;
  final ValueChanged<ImagePageMargin> onMarginChanged;

  bool get _isFit => pageSize == ImagePageSize.fit;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Page size', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        SegmentedButton<ImagePageSize>(
          key: const ValueKey('page-size-control'),
          expandedInsets: EdgeInsets.zero,
          showSelectedIcon: false,
          segments: const [
            ButtonSegment(
              value: ImagePageSize.fit,
              label: _SegmentLabel('Fit'),
            ),
            ButtonSegment(value: ImagePageSize.a4, label: _SegmentLabel('A4')),
            ButtonSegment(
              value: ImagePageSize.usLetter,
              label: _SegmentLabel('US Letter'),
            ),
          ],
          selected: {pageSize},
          onSelectionChanged: enabled
              ? (values) => onPageSizeChanged(values.single)
              : null,
        ),
        const SizedBox(height: 16),
        Text('Orientation', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        SegmentedButton<ImagePageOrientation>(
          key: const ValueKey('orientation-control'),
          expandedInsets: EdgeInsets.zero,
          showSelectedIcon: false,
          segments: const [
            ButtonSegment(
              value: ImagePageOrientation.portrait,
              label: _SegmentLabel('Portrait'),
            ),
            ButtonSegment(
              value: ImagePageOrientation.landscape,
              label: _SegmentLabel('Landscape'),
            ),
          ],
          selected: {orientation},
          onSelectionChanged: enabled && !_isFit
              ? (values) => onOrientationChanged(values.single)
              : null,
        ),
        const SizedBox(height: 16),
        Text('Margin', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        SegmentedButton<ImagePageMargin>(
          key: const ValueKey('margin-control'),
          expandedInsets: EdgeInsets.zero,
          showSelectedIcon: false,
          segments: const [
            ButtonSegment(
              value: ImagePageMargin.none,
              label: _SegmentLabel('No margin'),
            ),
            ButtonSegment(
              value: ImagePageMargin.small,
              label: _SegmentLabel('Small'),
            ),
            ButtonSegment(
              value: ImagePageMargin.big,
              label: _SegmentLabel('Big'),
            ),
          ],
          selected: {margin},
          onSelectionChanged: enabled && !_isFit
              ? (values) => onMarginChanged(values.single)
              : null,
        ),
        if (_isFit) ...[
          const SizedBox(height: 8),
          const Text('Fit uses each image’s own page shape with no margin.'),
        ],
      ],
    );
  }
}

class _SegmentLabel extends StatelessWidget {
  const _SegmentLabel(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return FittedBox(
      fit: BoxFit.scaleDown,
      child: Text(text, maxLines: 1, softWrap: false),
    );
  }
}

class _CreationFeedback extends StatelessWidget {
  const _CreationFeedback({required this.update});

  final ImagePdfCreationUpdate update;

  @override
  Widget build(BuildContext context) {
    return switch (update.status) {
      ImagePdfCreationStatus.running => Column(
        key: const ValueKey('feedback-running'),
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          LinearProgressIndicator(
            value: update.totalImageCount == 0
                ? null
                : update.completedImageCount / update.totalImageCount,
          ),
          const SizedBox(height: 8),
          Text(
            'Processing image ${update.currentImage ?? update.totalImageCount} of ${update.totalImageCount} · '
            '${update.completedImageCount} completed',
          ),
        ],
      ),
      ImagePdfCreationStatus.complete => _FeedbackMessage(
        message:
            'PDF creation complete: ${update.outputFiles.length} ${update.outputFiles.length == 1 ? 'file' : 'files'} created.',
        isError: false,
      ),
      ImagePdfCreationStatus.failed => _FeedbackMessage(
        message: _failureMessage(update),
        isError: true,
      ),
    };
  }

  String _failureMessage(ImagePdfCreationUpdate update) {
    final image = update.currentImage == null
        ? ''
        : ' at image ${update.currentImage}';
    final created = update.outputFiles.isEmpty
        ? 'No PDFs were created.'
        : '${update.outputFiles.length} PDFs were created before the failure.';
    return 'PDF creation failed$image. ${update.error?.message ?? 'Unable to complete PDF creation.'} $created';
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
