import 'dart:async';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';

typedef DroppedPathsCallback = Future<void> Function(List<String> paths);

class FileDropZone extends StatefulWidget {
  const FileDropZone({
    required this.child,
    required this.onDroppedPaths,
    this.onTap,
    this.enabled = true,
    super.key,
  });

  final Widget child;
  final DroppedPathsCallback onDroppedPaths;
  final VoidCallback? onTap;
  final bool enabled;

  @override
  State<FileDropZone> createState() => _FileDropZoneState();
}

class _FileDropZoneState extends State<FileDropZone> {
  bool _isDraggingOver = false;
  bool _isHovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final highlighted = widget.enabled && (_isDraggingOver || _isHovered);
    return DropTarget(
      key: const ValueKey('native-file-drop-target'),
      enable: widget.enabled,
      onDragEntered: (_) => setState(() => _isDraggingOver = true),
      onDragExited: (_) => setState(() => _isDraggingOver = false),
      onDragDone: (details) {
        setState(() => _isDraggingOver = false);
        final paths = details.files
            .map((file) => file.path)
            .where((path) => path.isNotEmpty)
            .toList(growable: false);
        if (paths.isNotEmpty) {
          unawaited(widget.onDroppedPaths(paths));
        }
      },
      child: MouseRegion(
        cursor: widget.onTap == null || !widget.enabled
            ? MouseCursor.defer
            : SystemMouseCursors.click,
        onEnter: (_) => setState(() => _isHovered = true),
        onExit: (_) => setState(() => _isHovered = false),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 140),
          curve: Curves.easeOut,
          decoration: BoxDecoration(
            color: highlighted
                ? colors.primaryContainer.withValues(alpha: 0.38)
                : Colors.transparent,
            border: Border.all(
              color: _isDraggingOver ? colors.primary : Colors.transparent,
              width: 2,
            ),
            borderRadius: BorderRadius.circular(17),
          ),
          child: Material(
            type: MaterialType.transparency,
            child: InkWell(
              onTap: widget.enabled ? widget.onTap : null,
              borderRadius: BorderRadius.circular(17),
              child: widget.child,
            ),
          ),
        ),
      ),
    );
  }
}

class EmptyFileDropContent extends StatelessWidget {
  const EmptyFileDropContent({
    required this.title,
    required this.formats,
    required this.actionLabel,
    required this.onAction,
    required this.icon,
    this.enabled = true,
    super.key,
  });

  final String title;
  final String formats;
  final String actionLabel;
  final VoidCallback onAction;
  final IconData icon;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(36),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            DecoratedBox(
              decoration: BoxDecoration(
                color: colors.primaryContainer,
                shape: BoxShape.circle,
              ),
              child: Padding(
                padding: const EdgeInsets.all(18),
                child: Icon(icon, size: 36, color: colors.primary),
              ),
            ),
            const SizedBox(height: 18),
            Text(
              title,
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 8),
            Text(
              formats,
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodyMedium
                  ?.copyWith(color: colors.onSurfaceVariant),
            ),
            const SizedBox(height: 20),
            OutlinedButton.icon(
              onPressed: enabled ? onAction : null,
              icon: const Icon(Icons.add_rounded),
              label: Text(actionLabel),
            ),
          ],
        ),
      ),
    );
  }
}
