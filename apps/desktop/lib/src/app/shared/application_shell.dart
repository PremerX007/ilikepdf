import 'package:flutter/material.dart';

class ApplicationShell extends StatelessWidget {
  const ApplicationShell({
    required this.applicationName,
    required this.coreVersion,
    required this.child,
    this.onBack,
    super.key,
  });

  final String applicationName;
  final String coreVersion;
  final Widget child;
  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Scaffold(
      key: const ValueKey('application-shell'),
      backgroundColor: colors.surfaceContainerLowest,
      appBar: AppBar(
        key: const ValueKey('application-header'),
        toolbarHeight: 64,
        automaticallyImplyLeading: false,
        leadingWidth: 56,
        leading: onBack != null
            ? IconButton(
                key: const ValueKey('back-home-button'),
                tooltip: 'Back to Home',
                onPressed: onBack,
                icon: const Icon(Icons.arrow_back_rounded),
              )
            : const SizedBox.shrink(),
        titleSpacing: 0,
        title: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            DecoratedBox(
              decoration: BoxDecoration(
                color: colors.primary,
                borderRadius: BorderRadius.circular(10),
              ),
              child: Padding(
                padding: const EdgeInsets.all(7),
                child: Icon(
                  Icons.description_rounded,
                  size: 20,
                  color: colors.onPrimary,
                ),
              ),
            ),
            const SizedBox(width: 10),
            Text(
              applicationName,
              key: const ValueKey('application-title'),
              style: const TextStyle(fontWeight: FontWeight.w700),
            ),
          ],
        ),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 24),
            child: Text(
              'Version $coreVersion',
              key: const ValueKey('application-version'),
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: colors.onSurfaceVariant),
            ),
          ),
        ],
      ),
      body: child,
    );
  }
}
