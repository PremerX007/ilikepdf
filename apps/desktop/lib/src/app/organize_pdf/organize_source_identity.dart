import 'package:flutter/material.dart';

/// An insertion slot is allocated once and never renumbered on removal/reset.
/// Large sessions use combinations of the same restrained palette rather than
/// random colors or silently assigning two sources the same identity.
@immutable
class OrganizeSourceAccent {
  const OrganizeSourceAccent(this.index) : assert(index >= 0);

  final int index;

  static const _palette = [
    Color(0xFF426BA8), // Blue
    Color(0xFF997323), // Ochre
    Color(0xFF32796F), // Teal
    Color(0xFF7860A1), // Violet
    Color(0xFFA65370), // Rose
    Color(0xFF65743C), // Olive
    Color(0xFF577584), // Slate
    Color(0xFF916648), // Clay
    Color(0xFF5C64A6), // Indigo
    Color(0xFF3F794D), // Forest
    Color(0xFF9D5942), // Terracotta
    Color(0xFF8A6284), // Mauve
  ];

  List<Color> get colors {
    var slot = index;
    final result = <Color>[];
    do {
      result.add(_palette[slot % _palette.length]);
      slot ~/= _palette.length;
    } while (slot > 0);
    return List.unmodifiable(result);
  }

  Color get primaryColor => _palette[index % _palette.length];
}

/// The numbered badge and filename description also identify sources without
/// color. This same widget is used in cards, drag feedback, and the source list.
class OrganizeSourceIdentity extends StatelessWidget {
  const OrganizeSourceIdentity({
    required this.accent,
    required this.sourceName,
    super.key,
  });

  final OrganizeSourceAccent accent;
  final String sourceName;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final label = 'Source ${accent.index + 1}: $sourceName';
    return Tooltip(
      message: label,
      excludeFromSemantics: true,
      child: Semantics(
        label: label,
        excludeSemantics: true,
        child: Container(
          height: 26,
          clipBehavior: Clip.antiAlias,
          decoration: BoxDecoration(
            color: accent.primaryColor.withValues(alpha: 0.10),
            borderRadius: BorderRadius.circular(5),
            border: Border.all(
              color: accent.primaryColor.withValues(alpha: 0.65),
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (final color in accent.colors)
                SizedBox(width: 3, child: ColoredBox(color: color)),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 7),
                child: Center(
                  child: Text(
                    '${accent.index + 1}',
                    style: Theme.of(context).textTheme.labelSmall
                        ?.copyWith(color: colors.onSurface),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
