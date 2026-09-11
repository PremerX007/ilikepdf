import 'package:flutter/material.dart';

import 'package:ilikepdf/src/app/home/tool_home.dart';
import 'package:ilikepdf/src/app/image_to_pdf/image_to_pdf_panel.dart';
import 'package:ilikepdf/src/app/image_to_pdf/image_to_pdf_workflow.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_panel.dart';
import 'package:ilikepdf/src/app/pdf_to_image/pdf_to_image_workflow.dart';
import 'package:ilikepdf/src/app/shared/application_shell.dart';

abstract final class ToolRoutes {
  static const home = '/';
  static const pdfToImages = '/pdf-to-images';
  static const imagesToPdf = '/images-to-pdf';
}

class IlikepdfApp extends StatelessWidget {
  const IlikepdfApp({
    required this.applicationName,
    required this.coreVersion,
    required this.localOnly,
    required this.pdfToImageWorkflow,
    this.imageToPdfWorkflow = const LocalImageToPdfWorkflow(),
    super.key,
  });

  final String applicationName;
  final String coreVersion;
  final bool localOnly;
  final PdfToImageWorkflow pdfToImageWorkflow;
  final ImageToPdfWorkflow imageToPdfWorkflow;

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
      initialRoute: ToolRoutes.home,
      routes: {
        ToolRoutes.home: (context) => ApplicationShell(
          applicationName: applicationName,
          coreVersion: coreVersion,
          child: ToolHome(
            onOpenPdfToImages: () =>
                Navigator.of(context).pushNamed(ToolRoutes.pdfToImages),
            onOpenImagesToPdf: () =>
                Navigator.of(context).pushNamed(ToolRoutes.imagesToPdf),
          ),
        ),
        ToolRoutes.pdfToImages: (context) => ApplicationShell(
          applicationName: applicationName,
          coreVersion: coreVersion,
          showBackButton: true,
          child: PdfToImagePanel(workflow: pdfToImageWorkflow),
        ),
        ToolRoutes.imagesToPdf: (context) => ApplicationShell(
          applicationName: applicationName,
          coreVersion: coreVersion,
          showBackButton: true,
          child: ImageToPdfPanel(workflow: imageToPdfWorkflow),
        ),
      },
    );
  }
}
