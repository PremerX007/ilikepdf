import 'package:flutter/material.dart';

import 'package:ilikepdf/src/app/home/tool_home.dart';
import 'package:ilikepdf/src/app/image_to_pdf/image_to_pdf_panel.dart';
import 'package:ilikepdf/src/app/image_to_pdf/image_to_pdf_workflow.dart';
import 'package:ilikepdf/src/app/merge_pdf/merge_pdf_panel.dart';
import 'package:ilikepdf/src/app/merge_pdf/merge_pdf_workflow.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_panel.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';
import 'package:ilikepdf/src/app/shared/application_shell.dart';

class IlikepdfApp extends StatelessWidget {
  const IlikepdfApp({
    required this.applicationName,
    required this.coreVersion,
    required this.localOnly,
    required this.pdfToImageWorkflow,
    this.imageToPdfWorkflow = const LocalImageToPdfWorkflow(),
    this.mergePdfWorkflow = const LocalMergePdfWorkflow(),
    super.key,
  });

  final String applicationName;
  final String coreVersion;
  final bool localOnly;
  final PdfToImageWorkflow pdfToImageWorkflow;
  final ImageToPdfWorkflow imageToPdfWorkflow;
  final MergePdfWorkflow mergePdfWorkflow;

  @override
  Widget build(BuildContext context) {
    final scheme = ColorScheme.fromSeed(
      seedColor: const Color(0xFFB42318),
      brightness: Brightness.light,
      surface: const Color(0xFFFFFBFA),
    );
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: applicationName,
      theme: ThemeData(
        colorScheme: scheme,
        scaffoldBackgroundColor: scheme.surfaceContainerLowest,
        useMaterial3: true,
        appBarTheme: AppBarTheme(
          backgroundColor: scheme.surface,
          foregroundColor: scheme.onSurface,
          surfaceTintColor: Colors.transparent,
          elevation: 0,
          shape: Border(bottom: BorderSide(color: scheme.outlineVariant)),
        ),
        filledButtonTheme: FilledButtonThemeData(
          style: FilledButton.styleFrom(
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
            ),
          ),
        ),
        outlinedButtonTheme: OutlinedButtonThemeData(
          style: OutlinedButton.styleFrom(
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
            ),
          ),
        ),
      ),
      home: _ApplicationWorkspace(
        applicationName: applicationName,
        coreVersion: coreVersion,
        pdfToImageWorkflow: pdfToImageWorkflow,
        imageToPdfWorkflow: imageToPdfWorkflow,
        mergePdfWorkflow: mergePdfWorkflow,
      ),
    );
  }
}

enum _ActiveTool { home, pdfToImages, imagesToPdf, mergePdf }

class _ApplicationWorkspace extends StatefulWidget {
  const _ApplicationWorkspace({
    required this.applicationName,
    required this.coreVersion,
    required this.pdfToImageWorkflow,
    required this.imageToPdfWorkflow,
    required this.mergePdfWorkflow,
  });

  final String applicationName;
  final String coreVersion;
  final PdfToImageWorkflow pdfToImageWorkflow;
  final ImageToPdfWorkflow imageToPdfWorkflow;
  final MergePdfWorkflow mergePdfWorkflow;

  @override
  State<_ApplicationWorkspace> createState() => _ApplicationWorkspaceState();
}

class _ApplicationWorkspaceState extends State<_ApplicationWorkspace> {
  static const _transitionDuration = Duration(milliseconds: 140);

  _ActiveTool _activeTool = _ActiveTool.home;

  void _show(_ActiveTool tool) {
    if (_activeTool != tool) {
      setState(() => _activeTool = tool);
    }
  }

  @override
  Widget build(BuildContext context) {
    final reduceMotion =
        MediaQuery.maybeOf(context)?.disableAnimations ?? false;
    final duration = reduceMotion ? Duration.zero : _transitionDuration;

    return ApplicationShell(
      applicationName: widget.applicationName,
      coreVersion: widget.coreVersion,
      onBack: _activeTool == _ActiveTool.home
          ? null
          : () => _show(_ActiveTool.home),
      child: AnimatedSwitcher(
        key: const ValueKey('primary-content-switcher'),
        duration: duration,
        reverseDuration: duration,
        switchInCurve: Curves.easeOutCubic,
        switchOutCurve: Curves.easeInCubic,
        transitionBuilder: (child, animation) =>
            FadeTransition(opacity: animation, child: child),
        layoutBuilder: (currentChild, previousChildren) => Stack(
          fit: StackFit.expand,
          children: [...previousChildren, ?currentChild],
        ),
        child: _buildActiveContent(),
      ),
    );
  }

  Widget _buildActiveContent() {
    return switch (_activeTool) {
      _ActiveTool.home => ToolHome(
        key: const ValueKey('home-content'),
        onOpenPdfToImages: () => _show(_ActiveTool.pdfToImages),
        onOpenImagesToPdf: () => _show(_ActiveTool.imagesToPdf),
        onOpenMergePdf: () => _show(_ActiveTool.mergePdf),
      ),
      _ActiveTool.pdfToImages => PdfToImagePanel(
        key: const ValueKey('pdf-to-images-content'),
        workflow: widget.pdfToImageWorkflow,
      ),
      _ActiveTool.imagesToPdf => ImageToPdfPanel(
        key: const ValueKey('images-to-pdf-content'),
        workflow: widget.imageToPdfWorkflow,
      ),
      _ActiveTool.mergePdf => MergePdfPanel(
        key: const ValueKey('merge-pdf-content'),
        workflow: widget.mergePdfWorkflow,
      ),
    };
  }
}
