import 'package:flutter/material.dart';

import 'package:ilikepdf/src/app/app.dart';
import 'package:ilikepdf/src/app/pdf_preview/pdf_preview_workflow.dart';
import 'package:ilikepdf/src/rust/api/application.dart';
import 'package:ilikepdf/src/rust/frb_generated.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final applicationInfo = await getApplicationInfo();

  runApp(
    IlikepdfApp(
      applicationName: applicationInfo.name,
      coreVersion: applicationInfo.version,
      localOnly: applicationInfo.localOnly,
      pdfWorkflow: const LocalPdfPreviewWorkflow(),
    ),
  );
}
