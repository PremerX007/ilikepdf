import 'package:flutter/material.dart';

class FilePreviewCard extends StatefulWidget {
  const FilePreviewCard({
    required this.thumbnail,
    required this.filename,
    required this.positionLabel,
    this.reorderHandle,
    required this.onRemove,
    this.removeButtonKey,
    this.isDropTarget = false,
    super.key,
  });

  final Widget thumbnail;
  final String filename;
  final String positionLabel;
  final Widget? reorderHandle;
  final VoidCallback? onRemove;
  final Key? removeButtonKey;
  final bool isDropTarget;

  @override
  State<FilePreviewCard> createState() => _FilePreviewCardState();
}

class _FilePreviewCardState extends State<FilePreviewCard> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final emphasized = _hovered || widget.isDropTarget;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 120),
        curve: Curves.easeOut,
        decoration: BoxDecoration(
          color: colors.surface,
          borderRadius: BorderRadius.circular(15),
          border: Border.all(
            color: emphasized ? colors.primary : colors.outlineVariant,
            width: emphasized ? 1.5 : 1,
          ),
          boxShadow: emphasized
              ? [
                  BoxShadow(
                    color: colors.shadow.withValues(alpha: 0.08),
                    blurRadius: 14,
                    offset: const Offset(0, 5),
                  ),
                ]
              : null,
        ),
        child: Padding(
          padding: const EdgeInsets.all(10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Expanded(
                child: Stack(
                  children: [
                    Positioned.fill(
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(10),
                        child: ColoredBox(
                          color: colors.surfaceContainerHighest,
                          child: widget.thumbnail,
                        ),
                      ),
                    ),
                    if (widget.reorderHandle case final reorderHandle?)
                      Positioned(
                        top: 6,
                        left: 6,
                        child: _CardControl(child: reorderHandle),
                      ),
                    Positioned(
                      top: 6,
                      right: 6,
                      child: _CardControl(
                        child: IconButton(
                          key: widget.removeButtonKey,
                          tooltip: 'Remove ${widget.filename}',
                          visualDensity: VisualDensity.compact,
                          onPressed: widget.onRemove,
                          icon: const Icon(Icons.close_rounded, size: 18),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 9),
              Text(
                widget.filename,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.labelLarge,
              ),
              const SizedBox(height: 2),
              Text(
                widget.positionLabel,
                style: Theme.of(context).textTheme.bodySmall
                    ?.copyWith(color: colors.onSurfaceVariant),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _CardControl extends StatelessWidget {
  const _CardControl({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.94),
        borderRadius: BorderRadius.circular(8),
      ),
      child: SizedBox.square(dimension: 34, child: Center(child: child)),
    );
  }
}
