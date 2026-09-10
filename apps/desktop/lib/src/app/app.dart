import 'package:flutter/material.dart';

import 'package:ilikepdf/src/app/pdf_preview/pdf_preview_panel.dart';
import 'package:ilikepdf/src/app/pdf_preview/pdf_preview_workflow.dart';

class IlikepdfApp extends StatelessWidget {
  const IlikepdfApp({
    required this.applicationName,
    required this.coreVersion,
    required this.localOnly,
    required this.pdfWorkflow,
    super.key,
  });

  final String applicationName;
  final String coreVersion;
  final bool localOnly;
  final PdfPreviewWorkflow pdfWorkflow;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: applicationName,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFFB42318)),
        useMaterial3: true,
      ),
      home: Scaffold(
        appBar: AppBar(title: Text(applicationName)),
        body: PdfPreviewPanel(workflow: pdfWorkflow),
        bottomNavigationBar: Padding(
          padding: const EdgeInsets.all(12),
          child: Text(
            '${localOnly ? 'Privacy mode: local processing only' : 'Privacy mode unavailable'} · Rust core $coreVersion',
            textAlign: TextAlign.center,
          ),
        ),
      ),
    );
  }
}
