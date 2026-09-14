import 'package:flutter/material.dart';

class ToolHome extends StatelessWidget {
  const ToolHome({
    required this.onOpenPdfToImages,
    required this.onOpenImagesToPdf,
    required this.onOpenMergePdf,
    super.key,
  });

  final VoidCallback onOpenPdfToImages;
  final VoidCallback onOpenImagesToPdf;
  final VoidCallback onOpenMergePdf;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(28, 38, 28, 42),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1120),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                'PDF tools',
                style: Theme.of(context).textTheme.headlineLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                  letterSpacing: -0.8,
                ),
              ),
              const SizedBox(height: 8),
              Text(
                'Choose a tool to get started.',
                style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 28),
              _ToolCardGrid(
                cards: [
                  _ToolCardData(
                    key: const ValueKey('tool-card-pdf-to-images'),
                    icon: Icons.image_outlined,
                    title: 'PDF to Images',
                    description: 'Export every PDF page as a clear PNG image.',
                    onTap: onOpenPdfToImages,
                  ),
                  _ToolCardData(
                    key: const ValueKey('tool-card-images-to-pdf'),
                    icon: Icons.picture_as_pdf_outlined,
                    title: 'Images to PDF',
                    description:
                        'Arrange JPG, PNG, and WebP files into polished PDFs.',
                    onTap: onOpenImagesToPdf,
                  ),
                  _ToolCardData(
                    key: ValueKey('tool-card-merge-pdf'),
                    icon: Icons.call_merge_rounded,
                    title: 'Merge PDF',
                    description:
                        'Combine PDF documents in the order you choose.',
                    onTap: onOpenMergePdf,
                  ),
                  const _ToolCardData(
                    key: ValueKey('tool-card-split-pdf'),
                    icon: Icons.content_cut_rounded,
                    title: 'Split PDF',
                    description: 'Extract selected pages into new documents.',
                  ),
                  const _ToolCardData(
                    key: ValueKey('tool-card-organize-pdf'),
                    icon: Icons.grid_view_rounded,
                    title: 'Organize PDF',
                    description: 'Reorder and remove pages visually.',
                  ),
                  const _ToolCardData(
                    key: ValueKey('tool-card-protect-pdf'),
                    icon: Icons.lock_outline_rounded,
                    title: 'Protect PDF',
                    description: 'Add password protection to a document.',
                  ),
                  const _ToolCardData(
                    key: ValueKey('tool-card-unlock-pdf'),
                    icon: Icons.lock_open_rounded,
                    title: 'Unlock PDF',
                    description:
                        'Remove protection when you know the password.',
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ToolCardData {
  const _ToolCardData({
    required this.key,
    required this.icon,
    required this.title,
    required this.description,
    this.onTap,
  });

  final Key key;
  final IconData icon;
  final String title;
  final String description;
  final VoidCallback? onTap;
}

class _ToolCardGrid extends StatelessWidget {
  const _ToolCardGrid({required this.cards});

  final List<_ToolCardData> cards;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        const spacing = 16.0;
        final columns = constraints.maxWidth >= 1000
            ? 3
            : constraints.maxWidth >= 660
            ? 2
            : 1;
        final width =
            (constraints.maxWidth - spacing * (columns - 1)) / columns;
        return Wrap(
          spacing: spacing,
          runSpacing: spacing,
          children: cards
              .map(
                (card) => SizedBox(
                  width: width,
                  height: 174,
                  child: _ToolCard(data: card),
                ),
              )
              .toList(growable: false),
        );
      },
    );
  }
}

class _ToolCard extends StatefulWidget {
  const _ToolCard({required this.data});

  final _ToolCardData data;

  @override
  State<_ToolCard> createState() => _ToolCardState();
}

class _ToolCardState extends State<_ToolCard> {
  bool _hovered = false;

  bool get _enabled => widget.data.onTap != null;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Semantics(
      button: _enabled,
      enabled: _enabled,
      child: MouseRegion(
        cursor: _enabled ? SystemMouseCursors.click : SystemMouseCursors.basic,
        onEnter: _enabled ? (_) => setState(() => _hovered = true) : null,
        onExit: _enabled ? (_) => setState(() => _hovered = false) : null,
        child: AnimatedContainer(
          key: widget.data.key,
          duration: const Duration(milliseconds: 140),
          transform: Matrix4.translationValues(0, _hovered ? -3 : 0, 0),
          decoration: BoxDecoration(
            color: _enabled ? colors.surface : colors.surfaceContainerLow,
            borderRadius: BorderRadius.circular(16),
            border: Border.all(
              color: _hovered ? colors.primary : colors.outlineVariant,
            ),
            boxShadow: _hovered
                ? [
                    BoxShadow(
                      color: colors.shadow.withValues(alpha: 0.09),
                      blurRadius: 18,
                      offset: const Offset(0, 8),
                    ),
                  ]
                : null,
          ),
          child: Material(
            type: MaterialType.transparency,
            child: InkWell(
              onTap: widget.data.onTap,
              borderRadius: BorderRadius.circular(16),
              child: Padding(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Icon(
                          widget.data.icon,
                          size: 28,
                          color: _enabled ? colors.primary : colors.outline,
                        ),
                        const Spacer(),
                        if (!_enabled)
                          DecoratedBox(
                            decoration: BoxDecoration(
                              color: colors.surfaceContainerHighest,
                              borderRadius: BorderRadius.circular(99),
                            ),
                            child: Padding(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 9,
                                vertical: 4,
                              ),
                              child: Text(
                                'Coming soon',
                                style: Theme.of(context).textTheme.labelSmall,
                              ),
                            ),
                          ),
                      ],
                    ),
                    const Spacer(),
                    Text(
                      widget.data.title,
                      style: Theme.of(context).textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                        color: _enabled ? null : colors.onSurfaceVariant,
                      ),
                    ),
                    const SizedBox(height: 5),
                    Text(
                      widget.data.description,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.bodyMedium
                          ?.copyWith(color: colors.onSurfaceVariant),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
