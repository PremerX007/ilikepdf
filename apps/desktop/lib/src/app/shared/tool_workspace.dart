import 'dart:math' as math;

import 'package:flutter/material.dart';

class ToolWorkspace extends StatelessWidget {
  const ToolWorkspace({
    required this.title,
    required this.description,
    required this.workspace,
    required this.settings,
    required this.primaryAction,
    this.status,
    super.key,
  });

  final String title;
  final String description;
  final Widget workspace;
  final Widget settings;
  final Widget primaryAction;
  final Widget? status;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(28, 22, 28, 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(title, style: Theme.of(context).textTheme.headlineSmall),
          const SizedBox(height: 4),
          Text(
            description,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 20),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constraints) {
                if (constraints.maxWidth >= 900) {
                  return Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Expanded(child: _WorkspaceSurface(child: workspace)),
                      const SizedBox(width: 20),
                      SizedBox(
                        width: 360,
                        child: ToolSettingsPanel(
                          settings: settings,
                          status: status,
                          primaryAction: primaryAction,
                        ),
                      ),
                    ],
                  );
                }

                final workspaceHeight = math.max(
                  340.0,
                  math.min(500.0, constraints.maxHeight * 0.56),
                );
                return SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      SizedBox(
                        height: workspaceHeight,
                        child: _WorkspaceSurface(child: workspace),
                      ),
                      const SizedBox(height: 16),
                      SizedBox(
                        height: 620,
                        child: ToolSettingsPanel(
                          settings: settings,
                          status: status,
                          primaryAction: primaryAction,
                        ),
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

class ToolSettingsPanel extends StatelessWidget {
  const ToolSettingsPanel({
    required this.settings,
    required this.primaryAction,
    this.status,
    super.key,
  });

  final Widget settings;
  final Widget primaryAction;
  final Widget? status;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Material(
      key: const ValueKey('tool-settings-panel'),
      color: colors.surface,
      elevation: 1,
      shadowColor: colors.shadow.withValues(alpha: 0.12),
      clipBehavior: Clip.antiAlias,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(18),
        side: BorderSide(color: colors.outlineVariant),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(22),
              child: settings,
            ),
          ),
          if (status case final visibleStatus?) ...[
            Divider(height: 1, color: colors.outlineVariant),
            ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 180),
              child: SingleChildScrollView(
                key: const ValueKey('tool-status-region'),
                padding: const EdgeInsets.fromLTRB(22, 14, 22, 18),
                child: visibleStatus,
              ),
            ),
          ],
          Divider(height: 1, color: colors.outlineVariant),
          Padding(padding: const EdgeInsets.all(22), child: primaryAction),
        ],
      ),
    );
  }
}

class PrimaryToolAction extends StatelessWidget {
  const PrimaryToolAction({
    required this.label,
    required this.icon,
    required this.onPressed,
    this.isRunning = false,
    this.runningLabel = 'Working…',
    super.key,
  });

  final String label;
  final IconData icon;
  final VoidCallback? onPressed;
  final bool isRunning;
  final String runningLabel;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      height: 48,
      child: FilledButton.icon(
        onPressed: isRunning ? null : onPressed,
        icon: isRunning
            ? const SizedBox.square(
                dimension: 18,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : Icon(icon),
        label: Text(isRunning ? runningLabel : label),
      ),
    );
  }
}

class _WorkspaceSurface extends StatelessWidget {
  const _WorkspaceSurface({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return DecoratedBox(
      key: const ValueKey('tool-workspace-surface'),
      decoration: BoxDecoration(
        color: colors.surfaceContainerLow,
        borderRadius: BorderRadius.circular(18),
        border: Border.all(color: colors.outlineVariant),
      ),
      child: ClipRRect(borderRadius: BorderRadius.circular(18), child: child),
    );
  }
}
