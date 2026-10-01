import 'dart:collection';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ilikepdf/src/app/organize_pdf/organize_pdf_workflow.dart';
import 'package:ilikepdf/src/app/organize_pdf/organize_source_identity.dart';
import 'package:ilikepdf/src/app/shared/destination_picker.dart';
import 'package:ilikepdf/src/app/shared/file_drop_zone.dart';
import 'package:ilikepdf/src/app/shared/reorderable_item_grid.dart';
import 'package:ilikepdf/src/app/shared/tool_workspace.dart';

class OrganizePdfPanel extends StatefulWidget {
  const OrganizePdfPanel({required this.workflow, super.key});

  final OrganizePdfWorkflow workflow;

  @override
  State<OrganizePdfPanel> createState() => _OrganizePdfPanelState();
}

class _OrganizeSourceItem {
  const _OrganizeSourceItem({
    required this.id,
    required this.pdf,
    required this.accent,
  });

  final int id;
  final SelectedOrganizePdf pdf;
  final OrganizeSourceAccent accent;
}

class _OrganizePageItem {
  const _OrganizePageItem({
    required this.id,
    required this.sourceId,
    required this.sourcePageIndex,
    required this.rotation,
  });

  final int id;
  final int sourceId;
  final int sourcePageIndex;
  final OrganizePageRotation rotation;

  _OrganizePageItem withRotation(OrganizePageRotation value) =>
      _OrganizePageItem(
        id: id,
        sourceId: sourceId,
        sourcePageIndex: sourcePageIndex,
        rotation: value,
      );
}

class _OrganizePdfPanelState extends State<OrganizePdfPanel> {
  static const _maximumCachedThumbnails = 32;
  static const _maximumConcurrentThumbnailRenders = 2;

  final List<_OrganizeSourceItem> _sources = [];
  final List<_OrganizePageItem> _pages = [];
  final List<_OrganizePageItem> _originalPages = [];
  final LinkedHashMap<int, RenderedOrganizePdfPage> _thumbnails =
      LinkedHashMap();
  final Set<int> _thumbnailRequests = {};
  final Set<int> _thumbnailFailures = {};
  final List<int> _thumbnailQueue = [];
  final TextEditingController _outputNameController = TextEditingController(
    text: 'organized.pdf',
  );

  int _nextSourceId = 0;
  int _nextPageItemId = 0;
  int _activeThumbnailRenders = 0;
  String? _destinationDirectory;
  bool _customDestination = false;
  bool _isSelecting = false;
  bool _isOrganizing = false;
  String? _interactionError;
  OrganizePdfUpdate? _organizeUpdate;

  bool get _isBusy => _isSelecting || _isOrganizing;
  String? get _normalizedOutputName =>
      normalizeOrganizeOutputName(_outputNameController.text);
  bool get _canOrganize =>
      !_isBusy &&
      _sources.isNotEmpty &&
      _pages.isNotEmpty &&
      _destinationDirectory != null &&
      _normalizedOutputName != null;

  @override
  void dispose() {
    final cachedPaths = _thumbnails.values
        .map((thumbnail) => thumbnail.outputPath)
        .toSet();
    for (final path in cachedPaths) {
      File(path).delete().ignore();
    }
    _outputNameController.dispose();
    super.dispose();
  }

  Future<void> _selectPdfs() async {
    if (_isBusy) return;
    await _addWith(
      () => widget.workflow.selectPdfs(existingSourcePaths: _sourcePaths()),
      failureMessage: 'The PDFs could not be selected.',
    );
  }

  Future<void> _acceptDroppedPaths(List<String> paths) async {
    if (_isBusy) return;
    await _addWith(
      () => widget.workflow.preparePdfPaths(
        existingSourcePaths: _sourcePaths(),
        candidateSourcePaths: paths,
      ),
      failureMessage: 'The dropped PDFs could not be added.',
    );
  }

  Future<void> _addWith(
    Future<List<SelectedOrganizePdf>> Function() load, {
    required String failureMessage,
  }) async {
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _organizeUpdate = null;
    });
    try {
      final additions = await load();
      if (mounted && additions.isNotEmpty) _appendSources(additions);
    } on OrganizePdfSelectionException catch (error) {
      if (mounted) setState(() => _interactionError = error.problem.message);
    } on Object {
      if (mounted) setState(() => _interactionError = failureMessage);
    } finally {
      if (mounted) setState(() => _isSelecting = false);
    }
  }

  void _appendSources(List<SelectedOrganizePdf> additions) {
    final wasEmpty = _sources.isEmpty;
    setState(() {
      for (final pdf in additions) {
        final sourceId = _nextSourceId++;
        _sources.add(
          _OrganizeSourceItem(
            id: sourceId,
            pdf: pdf,
            accent: OrganizeSourceAccent(sourceId),
          ),
        );
        for (
          var sourcePageIndex = 0;
          sourcePageIndex < pdf.pageCount;
          sourcePageIndex++
        ) {
          final page = _OrganizePageItem(
            id: _nextPageItemId++,
            sourceId: sourceId,
            sourcePageIndex: sourcePageIndex,
            rotation: OrganizePageRotation.none,
          );
          _pages.add(page);
          _originalPages.add(page);
        }
      }
      if (wasEmpty && !_customDestination) {
        _destinationDirectory = additions.first.sourceDirectory;
      }
      _interactionError = null;
      _organizeUpdate = null;
    });
  }

  List<String> _sourcePaths() =>
      _sources.map((source) => source.pdf.sourcePath).toList(growable: false);

  _OrganizeSourceItem? _sourceById(int sourceId) {
    for (final source in _sources) {
      if (source.id == sourceId) return source;
    }
    return null;
  }

  _OrganizePageItem? _pageById(int pageItemId) {
    for (final page in _pages) {
      if (page.id == pageItemId) return page;
    }
    return null;
  }

  Future<void> _chooseDestination() async {
    if (_isBusy || _sources.isEmpty) return;
    setState(() {
      _isSelecting = true;
      _interactionError = null;
      _organizeUpdate = null;
    });
    try {
      final destination = await widget.workflow.chooseDestinationDirectory();
      if (mounted && destination != null) {
        setState(() {
          _destinationDirectory = destination;
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

  void _removeSource(int sourceId) {
    if (_isBusy) return;
    final removedPageIds = _originalPages
        .where((page) => page.sourceId == sourceId)
        .map((page) => page.id)
        .toSet();
    final removedThumbnails = removedPageIds
        .map((pageId) => _thumbnails[pageId])
        .whereType<RenderedOrganizePdfPage>()
        .toList(growable: false);
    setState(() {
      _sources.removeWhere((source) => source.id == sourceId);
      _pages.removeWhere((page) => page.sourceId == sourceId);
      _originalPages.removeWhere((page) => page.sourceId == sourceId);
      for (final pageId in removedPageIds) {
        _thumbnails.remove(pageId);
        _thumbnailRequests.remove(pageId);
        _thumbnailFailures.remove(pageId);
      }
      _thumbnailQueue.removeWhere(removedPageIds.contains);
      _interactionError = null;
      _organizeUpdate = null;
      if (_sources.isEmpty) {
        _destinationDirectory = null;
        _customDestination = false;
        _outputNameController.text = 'organized.pdf';
      }
    });
    for (final thumbnail in removedThumbnails) {
      _deleteThumbnailIfUnreferenced(thumbnail);
    }
  }

  void _deletePage(int pageItemId) {
    if (_isBusy) return;
    final removedThumbnail = _thumbnails[pageItemId];
    setState(() {
      _pages.removeWhere((page) => page.id == pageItemId);
      _thumbnails.remove(pageItemId);
      _thumbnailRequests.remove(pageItemId);
      _thumbnailFailures.remove(pageItemId);
      _thumbnailQueue.remove(pageItemId);
      _interactionError = null;
      _organizeUpdate = null;
    });
    if (removedThumbnail != null) {
      _deleteThumbnailIfUnreferenced(removedThumbnail);
    }
  }

  void _rotatePage(int pageItemId, {required bool clockwise}) {
    if (_isBusy) return;
    final index = _pages.indexWhere((page) => page.id == pageItemId);
    if (index < 0) return;
    setState(() {
      final current = _pages[index];
      _pages[index] = current.withRotation(
        _nextRotation(current.rotation, clockwise: clockwise),
      );
      _interactionError = null;
      _organizeUpdate = null;
    });
  }

  void _reorderPage(int oldIndex, int newIndex) {
    if (_isBusy || oldIndex == newIndex) return;
    setState(() {
      final page = _pages.removeAt(oldIndex);
      _pages.insert(newIndex, page);
      _interactionError = null;
      _organizeUpdate = null;
    });
  }

  void _resetAll() {
    if (_isBusy || _sources.isEmpty) return;
    setState(() {
      _pages
        ..clear()
        ..addAll(_originalPages);
      _interactionError = null;
      _organizeUpdate = null;
    });
  }

  void _requestThumbnail(int pageItemId) {
    if (_thumbnails.containsKey(pageItemId)) {
      final cached = _thumbnails.remove(pageItemId)!;
      _thumbnails[pageItemId] = cached;
      return;
    }
    if (!_thumbnailRequests.add(pageItemId)) return;
    _thumbnailQueue.add(pageItemId);
    _drainThumbnailQueue();
  }

  void _drainThumbnailQueue() {
    while (_activeThumbnailRenders < _maximumConcurrentThumbnailRenders &&
        _thumbnailQueue.isNotEmpty) {
      final pageItemId = _thumbnailQueue.removeAt(0);
      final page = _pageById(pageItemId);
      final source = page == null ? null : _sourceById(page.sourceId);
      if (page == null || source == null) {
        _thumbnailRequests.remove(pageItemId);
        continue;
      }
      _activeThumbnailRenders++;
      widget.workflow
          .renderPage(
            sourcePath: source.pdf.sourcePath,
            pageIndex: page.sourcePageIndex,
          )
          .then((thumbnail) {
            if (!mounted || _pageById(pageItemId) == null) {
              File(thumbnail.outputPath).delete().ignore();
              return;
            }
            RenderedOrganizePdfPage? evicted;
            setState(() {
              while (_thumbnails.length >= _maximumCachedThumbnails) {
                evicted = _thumbnails.remove(_thumbnails.keys.first);
              }
              _thumbnails[pageItemId] = thumbnail;
              _thumbnailFailures.remove(pageItemId);
            });
            if (evicted != null) _deleteThumbnailIfUnreferenced(evicted!);
          })
          .catchError((Object _) {
            if (mounted && _pageById(pageItemId) != null) {
              setState(() => _thumbnailFailures.add(pageItemId));
            }
          })
          .whenComplete(() {
            _thumbnailRequests.remove(pageItemId);
            _activeThumbnailRenders--;
            if (mounted) _drainThumbnailQueue();
          });
    }
  }

  void _deleteThumbnailIfUnreferenced(RenderedOrganizePdfPage thumbnail) {
    if (_thumbnails.values.any(
      (cached) => cached.outputPath == thumbnail.outputPath,
    )) {
      return;
    }
    File(thumbnail.outputPath).delete().ignore();
  }

  Future<void> _organize() async {
    final outputName = _normalizedOutputName;
    final destination = _destinationDirectory;
    if (!_canOrganize || outputName == null || destination == null) return;
    setState(() {
      _isOrganizing = true;
      _interactionError = null;
      _outputNameController.text = outputName;
      _organizeUpdate = OrganizePdfUpdate(
        status: OrganizePdfUpdateStatus.running,
        stage: OrganizePdfProgressStage.preparing,
        sourceCount: _sources.length,
        pageCount: _pages.length,
        outputPath: null,
        warningSourceCount: 0,
        hasWarnings: false,
        failedSourceId: null,
        failedSourcePath: null,
        failedPageItemId: null,
        error: null,
      );
    });
    try {
      await for (final update in widget.workflow.organize(
        sources: _sources
            .map(
              (source) => OrganizePdfSourceInput(
                sourceId: source.id,
                sourcePath: source.pdf.sourcePath,
                pageCount: source.pdf.pageCount,
                hasWarnings: source.pdf.hasWarnings,
              ),
            )
            .toList(growable: false),
        pageItems: _pages
            .map(
              (page) => OrganizePdfPageInput(
                pageItemId: page.id,
                sourceId: page.sourceId,
                sourcePageIndex: page.sourcePageIndex,
                rotation: page.rotation,
              ),
            )
            .toList(growable: false),
        destinationDirectory: destination,
        outputName: outputName,
      )) {
        if (mounted) setState(() => _organizeUpdate = update);
      }
      if (mounted &&
          _organizeUpdate?.status == OrganizePdfUpdateStatus.running) {
        setState(
          () => _interactionError = 'The organize job ended without a result.',
        );
      }
    } on Object {
      if (mounted) {
        setState(() => _interactionError = 'The PDF could not be organized.');
      }
    } finally {
      if (mounted) setState(() => _isOrganizing = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ToolWorkspace(
      title: 'Organize PDF',
      description: 'Arrange pages from multiple PDFs, remove pages, and rotate them without reducing quality.',
      workspace: FileDropZone(
        enabled: !_isBusy,
        onDroppedPaths: _acceptDroppedPaths,
        onTap: _sources.isEmpty && !_isBusy ? _selectPdfs : null,
        child: _sources.isEmpty
            ? EmptyFileDropContent(
                key: const ValueKey('empty-organize-pdf-drop-zone'),
                title: 'Drop PDFs here',
                formats: 'PDF · Add one or more documents',
                actionLabel: 'Select PDFs',
                onAction: _selectPdfs,
                icon: Icons.grid_view_rounded,
                enabled: !_isBusy,
              )
            : _buildPageWorkspace(),
      ),
      settings: _buildSettings(),
      status: _buildStatus(),
      primaryAction: PrimaryToolAction(
        key: const ValueKey('organize-pdf-action'),
        label: 'Organize PDF',
        runningLabel: 'Organizing PDF…',
        icon: Icons.grid_view_rounded,
        isRunning: _isOrganizing,
        onPressed: _canOrganize ? _organize : null,
      ),
    );
  }

  Widget _buildPageWorkspace() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 14, 12),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  '${_pages.length} ${_pages.length == 1 ? 'page' : 'pages'} from ${_sources.length} ${_sources.length == 1 ? 'PDF' : 'PDFs'}',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              Text(
                'Drag pages to reorder',
                style: Theme.of(context).textTheme.bodySmall,
              ),
              const SizedBox(width: 12),
              OutlinedButton.icon(
                key: const ValueKey('add-organize-pdfs'),
                onPressed: _isBusy ? null : _selectPdfs,
                icon: const Icon(Icons.add_rounded),
                label: const Text('Add PDFs'),
              ),
            ],
          ),
        ),
        const Divider(height: 1),
        if (_pages.isEmpty)
          Expanded(
            child: Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Icon(Icons.layers_clear_outlined, size: 38),
                    const SizedBox(height: 12),
                    Text(
                      'No pages remain',
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    const SizedBox(height: 6),
                    const Text('Use Reset all to restore the source pages.'),
                  ],
                ),
              ),
            ),
          )
        else
          Expanded(
            child: LazyReorderableItemGrid<_OrganizePageItem>(
              items: _pages,
              itemKey: (page) => ValueKey(page.id),
              enabled: !_isBusy,
              onReorder: _reorderPage,
              itemBuilder: (context, page, index, dragSurface) {
                final source = _sourceById(page.sourceId)!;
                return _OrganizePageCard(
                  key: ValueKey('organize-page-card-${page.id}'),
                  page: page,
                  sourceName: source.pdf.displayName,
                  sourceAccent: source.accent,
                  thumbnail: _thumbnails[page.id],
                  previewFailed: _thumbnailFailures.contains(page.id),
                  enabled: !_isBusy,
                  dragSurface: dragSurface,
                  onThumbnailNeeded: () => _requestThumbnail(page.id),
                  onRotateLeft: () => _rotatePage(page.id, clockwise: false),
                  onRotateRight: () => _rotatePage(page.id, clockwise: true),
                  onDelete: () => _deletePage(page.id),
                );
              },
            ),
          ),
      ],
    );
  }

  Widget _buildSettings() {
    final nameIsInvalid =
        _outputNameController.text.isNotEmpty && _normalizedOutputName == null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Source PDFs',
                style: Theme.of(context).textTheme.titleMedium,
              ),
            ),
            TextButton(
              key: const ValueKey('reset-organize-all'),
              onPressed: _isBusy || _sources.isEmpty ? null : _resetAll,
              child: const Text('Reset all'),
            ),
          ],
        ),
        const SizedBox(height: 8),
        if (_sources.isEmpty)
          Text(
            'No source PDFs loaded',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          )
        else
          ..._sources.map(
            (source) => Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: DecoratedBox(
                key: ValueKey('organize-source-surface-${source.id}'),
                decoration: BoxDecoration(
                  color: source.accent.surfaceColor(
                    Theme.of(context).brightness,
                  ),
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(
                    color: source.accent.primaryColor.withValues(alpha: 0.65),
                  ),
                ),
                child: ListTile(
                  dense: true,
                  leading: OrganizeSourceIdentity(
                    key: ValueKey('organize-source-identity-${source.id}'),
                    accent: source.accent,
                    sourceName: source.pdf.displayName,
                  ),
                  title: Text(
                    source.pdf.displayName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  subtitle: Text(
                    '${source.pdf.pageCount} ${source.pdf.pageCount == 1 ? 'page' : 'pages'}',
                  ),
                  trailing: IconButton(
                    key: ValueKey('remove-organize-source-${source.id}'),
                    tooltip: 'Remove source PDF',
                    onPressed: _isBusy ? null : () => _removeSource(source.id),
                    icon: const Icon(Icons.close_rounded),
                  ),
                ),
              ),
            ),
          ),
        const SizedBox(height: 16),
        Text('Output filename', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        TextField(
          key: const ValueKey('organize-output-name'),
          controller: _outputNameController,
          enabled: !_isBusy,
          decoration: InputDecoration(
            border: const OutlineInputBorder(),
            isDense: true,
            errorText: nameIsInvalid ? 'Enter a filename, not a path.' : null,
          ),
          onChanged: (_) => setState(() {
            _interactionError = null;
            _organizeUpdate = null;
          }),
          onEditingComplete: () {
            final normalized = _normalizedOutputName;
            if (normalized != null) {
              setState(() => _outputNameController.text = normalized);
            }
            FocusScope.of(context).unfocus();
          },
        ),
        const SizedBox(height: 18),
        DestinationPicker(
          path: _destinationDirectory,
          placeholder: 'Add a PDF to choose the default folder',
          onChoose: _isBusy || _sources.isEmpty ? null : _chooseDestination,
        ),
        if (_destinationDirectory != null) ...[
          const SizedBox(height: 8),
          Text(
            _customDestination
                ? 'Custom destination stays selected as the session changes.'
                : 'Default: folder of the first PDF added.',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
        ],
      ],
    );
  }

  Widget? _buildStatus() {
    final error = _interactionError;
    if (error != null) {
      return _StatusMessage(
        key: const ValueKey('organize-interaction-failure'),
        icon: Icons.error_outline,
        title: 'Could not update the session',
        message: error,
        isError: true,
      );
    }
    final update = _organizeUpdate;
    if (update == null) {
      if (_sources.isNotEmpty && _pages.isEmpty) {
        return const _StatusMessage(
          icon: Icons.info_outline,
          title: 'Keep at least one page',
          message: 'Reset the workspace or add another PDF to continue.',
        );
      }
      return null;
    }
    return switch (update.status) {
      OrganizePdfUpdateStatus.running => _StatusMessage(
        key: const ValueKey('organize-running'),
        icon: Icons.sync_rounded,
        title: _stageLabel(update.stage),
        message:
            '${update.pageCount} ${update.pageCount == 1 ? 'page' : 'pages'} in this output plan',
      ),
      OrganizePdfUpdateStatus.complete => _StatusMessage(
        key: const ValueKey('organize-success'),
        icon: Icons.check_circle_outline,
        title: 'Organized successfully',
        message:
            '${update.sourceCount} ${update.sourceCount == 1 ? 'file' : 'files'} · ${update.pageCount} ${update.pageCount == 1 ? 'page' : 'pages'}\n${update.outputPath}',
      ),
      OrganizePdfUpdateStatus.failed => _StatusMessage(
        key: const ValueKey('organize-failure'),
        icon: Icons.error_outline,
        title: 'Organize failed',
        message: update.error?.message ?? 'The PDF could not be organized.',
        isError: true,
      ),
    };
  }
}

class _OrganizePageCard extends StatefulWidget {
  const _OrganizePageCard({
    required this.page,
    required this.sourceName,
    required this.sourceAccent,
    required this.thumbnail,
    required this.previewFailed,
    required this.enabled,
    required this.dragSurface,
    required this.onThumbnailNeeded,
    required this.onRotateLeft,
    required this.onRotateRight,
    required this.onDelete,
    super.key,
  });

  final _OrganizePageItem page;
  final String sourceName;
  final OrganizeSourceAccent sourceAccent;
  final RenderedOrganizePdfPage? thumbnail;
  final bool previewFailed;
  final bool enabled;
  final ReorderableDragBuilder dragSurface;
  final VoidCallback onThumbnailNeeded;
  final VoidCallback onRotateLeft;
  final VoidCallback onRotateRight;
  final VoidCallback onDelete;

  @override
  State<_OrganizePageCard> createState() => _OrganizePageCardState();
}

class _OrganizePageCardState extends State<_OrganizePageCard> {
  @override
  void initState() {
    super.initState();
    _requestIfNeeded();
  }

  @override
  void didUpdateWidget(covariant _OrganizePageCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.thumbnail == null && !widget.previewFailed) _requestIfNeeded();
  }

  void _requestIfNeeded() {
    if (widget.thumbnail != null || widget.previewFailed) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) widget.onThumbnailNeeded();
    });
  }

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Stack(
      fit: StackFit.expand,
      children: [
        widget.dragSurface(
          child: _buildCard(context, colors),
          feedback: Stack(
            fit: StackFit.expand,
            children: [_buildCard(context, colors), _buildActions()],
          ),
        ),
        // Siblings above the draggable are excluded by Stack hit testing,
        // including pointer movement beginning on an action button.
        _buildActions(),
      ],
    );
  }

  Widget _buildCard(BuildContext context, ColorScheme colors) {
    return Card(
      color: widget.sourceAccent.surfaceColor(colors.brightness),
      surfaceTintColor: Colors.transparent,
      margin: EdgeInsets.zero,
      clipBehavior: Clip.antiAlias,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(
          color: widget.sourceAccent.primaryColor.withValues(alpha: 0.65),
          width: 1.5,
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: ColoredBox(
              key: ValueKey('organize-page-preview-surface-${widget.page.id}'),
              color: widget.sourceAccent.previewColor(colors.brightness),
              child: Padding(
                padding: const EdgeInsets.all(10),
                child: Center(child: _buildThumbnail(colors)),
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 8, 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    OrganizeSourceIdentity(
                      key: ValueKey('organize-page-identity-${widget.page.id}'),
                      accent: widget.sourceAccent,
                      sourceName: widget.sourceName,
                    ),
                    const SizedBox(width: 7),
                    Expanded(
                      child: Tooltip(
                        message: widget.sourceName,
                        child: Text(
                          widget.sourceName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context).textTheme.labelLarge,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 2),
                Text(
                  key: ValueKey('organize-page-label-${widget.page.id}'),
                  'Page ${widget.page.sourcePageIndex + 1}',
                  style: Theme.of(context).textTheme.bodySmall
                      ?.copyWith(color: colors.onSurfaceVariant),
                ),
                const SizedBox(height: 6),
                const SizedBox(height: 32),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildActions() => Positioned(
    right: 8,
    bottom: 8,
    child: Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _PageAction(
          key: ValueKey('rotate-left-${widget.page.id}'),
          tooltip: 'Rotate left',
          icon: Icons.rotate_left_rounded,
          onPressed: widget.enabled ? widget.onRotateLeft : null,
        ),
        _PageAction(
          key: ValueKey('rotate-right-${widget.page.id}'),
          tooltip: 'Rotate right',
          icon: Icons.rotate_right_rounded,
          onPressed: widget.enabled ? widget.onRotateRight : null,
        ),
        _PageAction(
          key: ValueKey('delete-organize-page-${widget.page.id}'),
          tooltip: 'Delete page',
          icon: Icons.delete_outline_rounded,
          onPressed: widget.enabled ? widget.onDelete : null,
        ),
      ],
    ),
  );

  Widget _buildThumbnail(ColorScheme colors) {
    final thumbnail = widget.thumbnail;
    if (thumbnail != null) {
      return RotatedBox(
        key: ValueKey('organize-page-rotation-${widget.page.id}'),
        quarterTurns: _quarterTurns(widget.page.rotation),
        child: Image.file(
          File(thumbnail.outputPath),
          key: ValueKey('organize-page-thumbnail-${widget.page.id}'),
          fit: BoxFit.contain,
          gaplessPlayback: true,
        ),
      );
    }
    if (widget.previewFailed) {
      return Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.broken_image_outlined, color: colors.outline),
          const SizedBox(height: 6),
          const Text('Preview unavailable'),
        ],
      );
    }
    return const SizedBox.square(
      dimension: 24,
      child: CircularProgressIndicator(strokeWidth: 2),
    );
  }
}

class _PageAction extends StatelessWidget {
  const _PageAction({
    required this.tooltip,
    required this.icon,
    required this.onPressed,
    super.key,
  });

  final String tooltip;
  final IconData icon;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    return SizedBox.square(
      dimension: 32,
      child: IconButton(
        padding: EdgeInsets.zero,
        tooltip: tooltip,
        onPressed: onPressed,
        iconSize: 19,
        icon: Icon(icon),
      ),
    );
  }
}

class _StatusMessage extends StatelessWidget {
  const _StatusMessage({
    required this.icon,
    required this.title,
    required this.message,
    this.isError = false,
    super.key,
  });

  final IconData icon;
  final String title;
  final String message;
  final bool isError;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final color = isError ? colors.error : colors.primary;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, color: color, size: 20),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: Theme.of(context).textTheme.titleSmall),
              const SizedBox(height: 3),
              Text(
                message,
                style: Theme.of(context).textTheme.bodySmall
                    ?.copyWith(color: colors.onSurfaceVariant),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

OrganizePageRotation _nextRotation(
  OrganizePageRotation current, {
  required bool clockwise,
}) {
  const order = [
    OrganizePageRotation.none,
    OrganizePageRotation.clockwise90,
    OrganizePageRotation.halfTurn,
    OrganizePageRotation.counterClockwise90,
  ];
  final index = order.indexOf(current);
  return order[(index + (clockwise ? 1 : 3)) % order.length];
}

int _quarterTurns(OrganizePageRotation rotation) => switch (rotation) {
  OrganizePageRotation.none => 0,
  OrganizePageRotation.clockwise90 => 1,
  OrganizePageRotation.halfTurn => 2,
  OrganizePageRotation.counterClockwise90 => 3,
};

String _stageLabel(OrganizePdfProgressStage stage) => switch (stage) {
  OrganizePdfProgressStage.preparing => 'Preparing pages…',
  OrganizePdfProgressStage.organizing => 'Organizing PDF…',
  OrganizePdfProgressStage.validating => 'Validating output…',
  OrganizePdfProgressStage.publishing => 'Publishing…',
  OrganizePdfProgressStage.completed => 'Completed',
};
