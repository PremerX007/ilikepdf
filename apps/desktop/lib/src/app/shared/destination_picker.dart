import 'package:flutter/material.dart';

class DestinationPicker extends StatelessWidget {
  const DestinationPicker({
    required this.path,
    required this.placeholder,
    required this.onChoose,
    super.key,
  });

  final String? path;
  final String placeholder;
  final VoidCallback? onChoose;

  @override
  Widget build(BuildContext context) {
    final visiblePath = path ?? placeholder;
    return Column(
      key: const ValueKey('destination-section'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Destination', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        Tooltip(
          message: visiblePath,
          child: Text(
            visiblePath,
            key: const ValueKey('destination-path'),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
        ),
        const SizedBox(height: 10),
        OutlinedButton.icon(
          onPressed: onChoose,
          icon: const Icon(Icons.folder_open_outlined),
          label: const Text('Choose folder'),
        ),
      ],
    );
  }
}
